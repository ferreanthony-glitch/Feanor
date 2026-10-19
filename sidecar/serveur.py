"""Le serveur local de génération sonore de SHOT (« sidecar »).

Il implémente le contrat de `docs/GENERATION_SON.md` §6, rien de plus :

  GET  /v1/etat      quels modèles sont installés
  POST /v1/lecture   un plan du storyboard -> des indications de sons   (Qwen2.5-VL)
  POST /v1/son       un prompt -> un bruitage / une ambiance / une musique (Stable Audio Open)
  POST /v1/voix      une réplique -> une voix témoin                      (Kokoro-82M)

- Écoute sur 127.0.0.1 UNIQUEMENT (aucun réglage ne permet autre chose).
- Une demande à la fois ; une autre reçoit `503 occupe`. Fermer la connexion annule la demande en cours.
- Les modèles se chargent à la première demande ; un seul gros modèle reste dans la carte graphique.
- Aucun accès réseau : `HF_HUB_OFFLINE=1` et `TRANSFORMERS_OFFLINE=1` sont posés avant tout import.

Lancement (voir `sidecar/lancer.ps1`, ou SHOT qui le fait lui-même) :
    <python du venv> -I sidecar/serveur.py --racine <dossier SHOT-modeles> [--port 8765]
"""

import os

os.environ["HF_HUB_OFFLINE"] = "1"
os.environ["TRANSFORMERS_OFFLINE"] = "1"
os.environ["HF_HUB_DISABLE_TELEMETRY"] = "1"
os.environ["TOKENIZERS_PARALLELISM"] = "false"
os.environ["PYTHONUNBUFFERED"] = "1"

import argparse
import asyncio
import sys

# (Le journal est lu par SHOT en UTF-8, quelle que soit la page de code de la console Windows.)
for _flux in (sys.stdout, sys.stderr):
    if _flux is not None and hasattr(_flux, "reconfigure"):
        _flux.reconfigure(encoding="utf-8", errors="replace")
import base64
import json
import sys
import threading
import time
import tomllib

# (Avec `python -I`, le dossier du script n'est pas dans sys.path : on l'ajoute pour `modeles.py` et `awq.py`.)
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse, Response
from starlette.exceptions import HTTPException as StarletteHTTPException

import modeles
from modeles import Annule, Chemins, ErreurApi, Gestionnaire, journal

VERSION_CONTRAT = 1
PORT_PAR_DEFAUT = 8765
MAX_REQUETE = 20 * 1024 * 1024
GENRES_SON = {"bruitage", "ambiance", "musique"}


# ------------------------------------------------------------------------------------- configuration


def lire_arguments(argv=None):
    p = argparse.ArgumentParser(description="Serveur local de génération sonore de SHOT (127.0.0.1 seulement).")
    p.add_argument("--port", type=int, default=None, help=f"port (défaut {PORT_PAR_DEFAUT})")
    p.add_argument("--racine", default=None, help="dossier SHOT-modeles (ses modèles sont dans <racine>/modeles) ; sinon la variable SHOT_MODELES")
    p.add_argument("--modeles", default=None, help="dossier qui contient directement un dossier par modèle (remplace <racine>/modeles)")
    p.add_argument("--config", default=None, help="fichier de configuration (défaut : config.toml à côté du script)")
    p.add_argument("--parent-pid", type=int, default=0, help="numéro du processus de SHOT : le serveur s'arrête quand il disparaît")
    return p.parse_args(argv)


def charger_config(args):
    chemin = args.config or os.path.join(os.path.dirname(os.path.abspath(__file__)), "config.toml")
    cfg = {}
    if os.path.isfile(chemin):
        with open(chemin, "rb") as f:
            cfg = tomllib.load(f)
    m = cfg.get("modeles", {})
    racine = args.racine or os.environ.get("SHOT_MODELES") or m.get("racine") or ""
    dossier = args.modeles or os.environ.get("SHOT_MODELES_DOSSIER") or m.get("dossier") or (os.path.join(racine, "modeles") if racine else "")
    chemins = Chemins(
        dossier=dossier or None,
        lecture=m.get("lecture", "Qwen2.5-VL-7B-Instruct-AWQ"),
        voix=m.get("voix", "Kokoro-82M"),
        son=m.get("son", "stable-audio-open-small"),
        t5=m.get("t5", "t5-base"),
        voix_defaut=cfg.get("voix", {}).get("defaut", "ff_siwis"),
        max_tokens=int(cfg.get("lecture", {}).get("max_tokens", 300)),
    )
    port = args.port or int(cfg.get("serveur", {}).get("port", PORT_PAR_DEFAUT))
    inactivite = float(cfg.get("serveur", {}).get("decharger_apres_s", 600))
    return chemins, port, inactivite


# ----------------------------------------------------------------------------------- l'application


def erreur(statut, code, message):
    return JSONResponse(status_code=statut, content={"erreur": {"code": code, "message": message}})


async def lire_json(request):
    taille = request.headers.get("content-length", "")
    if taille.isdigit() and int(taille) > MAX_REQUETE:
        raise ErreurApi(413, "invalide", "La demande dépasse 20 Mo.")
    octets = bytearray()
    async for morceau in request.stream():
        octets += morceau
        if len(octets) > MAX_REQUETE:
            raise ErreurApi(413, "invalide", "La demande dépasse 20 Mo.")
    try:
        v = json.loads(bytes(octets).decode("utf-8"))
    except (ValueError, UnicodeDecodeError):
        raise ErreurApi(400, "invalide", "La demande n'est pas du JSON valide.")
    if not isinstance(v, dict):
        raise ErreurApi(400, "invalide", "La demande doit être un objet JSON.")
    version = v.get("version", VERSION_CONTRAT)
    if not isinstance(version, int) or version > VERSION_CONTRAT:
        raise ErreurApi(400, "invalide", f"Version du contrat non gérée ({version}) : ce serveur parle la version {VERSION_CONTRAT}.")
    return v


def creer_app(chemins, inactivite_s=600.0):
    app = FastAPI(docs_url=None, redoc_url=None, openapi_url=None)
    gestionnaire = Gestionnaire(chemins)
    occupe = threading.Lock()  # une demande à la fois

    # ---- erreurs en JSON

    @app.exception_handler(ErreurApi)
    async def _erreur_api(request, exc):
        return erreur(exc.statut, exc.code, exc.message)

    @app.exception_handler(StarletteHTTPException)
    async def _erreur_http(request, exc):
        if exc.status_code == 404:
            return erreur(404, "invalide", "Adresse inconnue : ce serveur parle /v1/etat, /v1/lecture, /v1/son et /v1/voix.")
        if exc.status_code == 405:
            return erreur(405, "invalide", "Méthode non permise pour cette adresse.")
        return erreur(exc.status_code, "invalide" if exc.status_code < 500 else "interne", "Demande refusée.")

    @app.exception_handler(Exception)
    async def _erreur_interne(request, exc):
        journal(f"ERREUR interne : {type(exc).__name__}: {exc}")
        return erreur(500, "interne", "Le serveur local a rencontré une panne. Son journal en dit plus.")

    # ---- le travail d'une demande, hors de la boucle (la boucle reste libre pour /v1/etat et pour voir la déconnexion)

    async def travailler(request, fonction):
        if not occupe.acquire(blocking=False):
            raise ErreurApi(503, "occupe", "Le serveur local travaille déjà sur une autre demande.")
        annule = threading.Event()

        def tache():
            try:
                return fonction(annule)
            finally:
                gestionnaire.derniere_activite = time.time()
                occupe.release()

        futur = asyncio.get_running_loop().run_in_executor(None, tache)
        while True:
            fini, _ = await asyncio.wait({futur}, timeout=0.25)
            if fini:
                break
            if await request.is_disconnected():
                annule.set()
                journal("connexion fermée : demande annulée")
                try:
                    await futur
                except BaseException:
                    pass
                return Response(status_code=499)
        try:
            return futur.result()
        except Annule:
            return Response(status_code=499)
        except ErreurApi:
            raise
        except Exception as e:
            if type(e).__name__ == "OutOfMemoryError":
                gestionnaire.tout_decharger()
                raise ErreurApi(503, "indisponible", "Plus assez de mémoire graphique pour cette demande. Ferme les programmes qui l'utilisent et réessaie.")
            raise

    # ---- GET /v1/etat

    @app.get("/v1/etat")
    async def etat():
        return {
            "version": VERSION_CONTRAT,
            "modeles": {"lecture": chemins.etat_lecture(), "son": chemins.etat_son(), "voix": chemins.etat_voix()},
        }

    # ---- POST /v1/lecture

    @app.post("/v1/lecture")
    async def lecture(request: Request):
        v = await lire_json(request)
        plan = v.get("plan")
        if not isinstance(plan, dict):
            raise ErreurApi(400, "invalide", "La demande n'a pas de « plan ».")
        vignette = None
        if v.get("vignette_png_b64"):
            try:
                vignette = base64.b64decode(v["vignette_png_b64"], validate=False)
            except Exception:
                raise ErreurApi(400, "invalide", "La vignette n'est pas du base64 valide.")
        # (Le champ « consignes » de SHOT est ignoré : le serveur a les siennes, voir modeles.CONSIGNES.)
        try:
            images = max(1, int(plan.get("images") or 1))
        except (TypeError, ValueError):
            raise ErreurApi(400, "invalide", "Le plan n'a pas de nombre d'images valable.")
        plan["images"] = images
        if not chemins.etat_lecture()["installe"]:
            raise ErreurApi(501, "non_installe", "Le modèle de lecture (Qwen2.5-VL) n'est pas installé.")

        def faire(annule):
            debut = time.time()
            lecteur = gestionnaire.exiger("lecture")
            texte, jetons, secondes = lecteur.lire(plan, vignette, annule)
            sons = modeles.sons_depuis_lignes(texte, images, int(plan.get("cadence") or 24))
            if not sons:
                # (Un modèle qui répond en JSON malgré la consigne n'est pas perdu.)
                sons = [o for o in modeles.nettoyer_indications(modeles.objets_json(texte), images) if o["genre"] != "voix"]
            if not sons and "none" not in texte.lower() and texte.strip():
                journal("réponse inexploitable : " + texte[:300].replace("\n", " "))
                raise ErreurApi(422, "invalide", "Le modèle n'a pas rendu de réponse exploitable pour ce plan.")
            voix = modeles.voix_depuis_dialogue(plan)
            indications = sorted(sons + voix, key=lambda i: i["image"])
            journal(f"lecture « {plan.get('titre', '?')} » : {secondes:.1f} s pour {jetons} jetons ({jetons / max(secondes, 1e-6):.1f} jetons/s), {len(sons)} son(s) + {len(voix)} voix, au total {time.time() - debut:.1f} s")
            return JSONResponse({"indications": indications})

        return await travailler(request, faire)

    # ---- POST /v1/son

    @app.post("/v1/son")
    async def son(request: Request):
        v = await lire_json(request)
        genre = str(v.get("genre", "bruitage")).strip().lower()
        prompt = str(v.get("prompt", "")).strip()
        if genre not in GENRES_SON:
            raise ErreurApi(400, "invalide", "Le genre doit être « bruitage », « ambiance » ou « musique ».")
        if not prompt or len(prompt) > 600:
            raise ErreurApi(400, "invalide", "Le prompt est vide ou dépasse 600 caractères.")
        try:
            duree = float(v.get("duree_s", 4.0))
        except (TypeError, ValueError):
            raise ErreurApi(400, "invalide", "La durée voulue (duree_s) doit être un nombre de secondes.")
        if not (duree == duree) or duree <= 0:
            raise ErreurApi(400, "invalide", "La durée voulue (duree_s) doit être positive.")
        if not chemins.etat_son()["installe"]:
            raise ErreurApi(501, "modele_absent", chemins.raison_son_absent())

        def faire(annule):
            debut = time.time()
            generateur = gestionnaire.exiger("son")
            audio, frequence = generateur.generer(prompt, duree, annule)
            wav = modeles.wav_octets(audio, frequence)
            journal(f"son « {prompt[:50]} » : {audio.shape[0] / frequence:.1f} s de son en {time.time() - debut:.1f} s")
            return Response(content=wav, media_type="audio/wav")

        return await travailler(request, faire)

    # ---- POST /v1/voix

    @app.post("/v1/voix")
    async def voix(request: Request):
        v = await lire_json(request)
        texte = str(v.get("texte", "")).strip()
        if not texte or len(texte) > 2000:
            raise ErreurApi(400, "invalide", "Le texte de la réplique est vide ou dépasse 2 000 caractères.")
        nom = str(v.get("voix", "") or "").strip() or chemins.voix_defaut
        if not _nom_voix_valide(nom):
            raise ErreurApi(400, "invalide", f"Nom de voix invalide : « {nom[:40]} ».")
        if not chemins.etat_voix()["installe"]:
            raise ErreurApi(501, "non_installe", "Le modèle de voix (Kokoro-82M) n'est pas installé.")
        dossier_voix = os.path.join(chemins.dossier_voix(), "voices")
        if not os.path.isfile(os.path.join(dossier_voix, nom + ".pt")):
            dispo = sorted(f[:-3] for f in os.listdir(dossier_voix) if f.endswith(".pt")) if os.path.isdir(dossier_voix) else []
            raise ErreurApi(400, "invalide", f"La voix « {nom} » n'est pas installée. Voix disponibles : {', '.join(dispo) or 'aucune'}.")

        def faire(annule):
            debut = time.time()
            kokoro = gestionnaire.exiger("voix")
            audio, frequence = kokoro.synthetiser(texte, nom, annule)
            wav = modeles.wav_octets(audio, frequence)
            duree = audio.shape[0] / frequence
            journal(f"voix {nom} « {texte[:40]} » : {duree:.1f} s de parole en {time.time() - debut:.2f} s")
            return Response(content=wav, media_type="audio/wav")

        return await travailler(request, faire)

    # ---- la mémoire graphique rendue quand on ne s'en sert plus

    def veilleur():
        while True:
            time.sleep(15)
            if inactivite_s > 0 and gestionnaire.charges and time.time() - gestionnaire.derniere_activite > inactivite_s:
                if occupe.acquire(blocking=False):
                    try:
                        journal(f"inactif depuis {inactivite_s:.0f} s : les modèles sont déchargés")
                        gestionnaire.tout_decharger()
                    finally:
                        occupe.release()

    threading.Thread(target=veilleur, daemon=True).start()
    app.state.gestionnaire = gestionnaire
    return app


def _nom_voix_valide(nom):
    import re

    return re.fullmatch(r"[a-z]{2}_[a-z0-9]{1,30}", nom) is not None


# --------------------------------------------------------------------------------------- lancement


def surveiller_parent(pid):
    """Quitte quand le processus de SHOT disparaît (Windows)."""
    import ctypes

    k = ctypes.windll.kernel32
    k.OpenProcess.restype = ctypes.c_void_p
    h = k.OpenProcess(0x00100000, False, pid)  # SYNCHRONIZE
    if not h:
        return
    k.WaitForSingleObject(ctypes.c_void_p(h), 0xFFFFFFFF)
    journal("SHOT est fermé : le serveur local s'arrête")
    os._exit(0)


def main(argv=None):
    import uvicorn

    args = lire_arguments(argv)
    chemins, port, inactivite = charger_config(args)
    if args.parent_pid and os.name == "nt":
        threading.Thread(target=surveiller_parent, args=(args.parent_pid,), daemon=True).start()
    app = creer_app(chemins, inactivite)
    journal(f"serveur local de SHOT sur 127.0.0.1:{port} — modèles dans {chemins.dossier or '(aucun dossier)'}")
    uvicorn.run(app, host="127.0.0.1", port=port, log_level="warning", access_log=False, workers=1)


if __name__ == "__main__":
    main()
