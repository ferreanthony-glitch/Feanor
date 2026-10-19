"""Essai de fumée du serveur local (à lancer à la main, pytest n'est pas installé) :

    <python du venv> -I sidecar/essai.py --racine <dossier SHOT-modeles> [--sans-lecture] [--garder]

Il lance `serveur.py` sur un port libre, essaie chaque adresse du contrat §6 (état, voix, son, lecture, erreurs,
« occupé », annulation par fermeture de la connexion), affiche les durées et la mémoire graphique, puis arrête
le serveur. Aucun accès réseau en dehors de 127.0.0.1.
"""

import argparse
import base64
import io
import json
import os
import socket
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.request

ICI = os.path.dirname(os.path.abspath(__file__))
MAX_TEXTE = 2000  # le serveur refuse un texte de voix plus long
echecs = []


def verifier(condition, nom, detail=""):
    print(("  OK     " if condition else "  ECHEC  ") + nom + (f"  [{detail}]" if detail else ""), flush=True)
    if not condition:
        echecs.append(nom)


def port_libre():
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    p = s.getsockname()[1]
    s.close()
    return p


def appel(port, methode, chemin, corps=None, delai=600):
    donnees = json.dumps(corps).encode() if corps is not None else None
    req = urllib.request.Request(f"http://127.0.0.1:{port}{chemin}", data=donnees, method=methode, headers={"Content-Type": "application/json", "Connection": "close"})
    try:
        with urllib.request.urlopen(req, timeout=delai) as r:
            return r.status, r.headers.get("content-type", ""), r.read()
    except urllib.error.HTTPError as e:
        return e.code, e.headers.get("content-type", ""), e.read()


def vram_mo():
    try:
        sortie = subprocess.run(["nvidia-smi", "--query-gpu=memory.used", "--format=csv,noheader,nounits"], capture_output=True, text=True, timeout=10).stdout
        return int(sortie.strip().splitlines()[0])
    except Exception:
        return None


def vignette_png():
    from PIL import Image, ImageDraw

    im = Image.new("RGB", (384, 216), (30, 36, 50))
    d = ImageDraw.Draw(im)
    d.rectangle([0, 150, 384, 216], fill=(60, 50, 40))
    d.rectangle([150, 60, 230, 150], fill=(110, 80, 50))
    d.ellipse([60, 20, 100, 60], fill=(230, 230, 200))
    for x in range(0, 384, 24):
        d.line([x, 0, x - 20, 40], fill=(120, 140, 200), width=1)
    tampon = io.BytesIO()
    im.save(tampon, format="PNG")
    return tampon.getvalue()


def essai_son_factice():
    """Stable Audio n'est pas installé : on essaie la plomberie de /v1/son (durée, WAV, décharge de Qwen, 501) avec
    un FAUX module `stable_audio_tools` et de faux fichiers. Ce n'est PAS un essai du vrai modèle."""
    import tempfile
    import types

    sys.path.insert(0, ICI)
    import torch
    from fastapi.testclient import TestClient

    racine = tempfile.mkdtemp(prefix="shot-sidecar-faux-")
    modeles_dir = os.path.join(racine, "modeles")
    for sous, fichiers in {"stable-audio-open-small": ["model_config.json", "model.safetensors"], "t5-base": ["config.json"]}.items():
        os.makedirs(os.path.join(modeles_dir, sous))
        for f in fichiers:
            with open(os.path.join(modeles_dir, sous, f), "w") as fh:
                fh.write(json.dumps({"sample_rate": 44100, "sample_size": 524288, "model": {"conditioning": {"configs": [{"type": "t5", "config": {"t5_model_name": "t5-base"}}]}}}) if f.endswith(".json") and "config" in f and sous != "t5-base" else "{}")

    class FauxModele(torch.nn.Module):
        def load_state_dict(self, *a, **k):
            return None

    vu = {}

    def create_model_from_config(config):
        vu["t5"] = config["model"]["conditioning"]["configs"][0]["config"]["t5_model_name"]
        return FauxModele()

    def generate_diffusion_cond(modele, steps, cfg_scale, conditioning, sample_size, sampler_type, device):
        vu["conditioning"] = conditioning
        return torch.sin(torch.arange(sample_size).float() * 0.01).repeat(1, 2, 1) * 0.5

    for nom in ("stable_audio_tools", "stable_audio_tools.models", "stable_audio_tools.inference"):
        sys.modules[nom] = types.ModuleType(nom)
    sys.modules["stable_audio_tools.models.factory"] = types.SimpleNamespace(create_model_from_config=create_model_from_config)
    sys.modules["stable_audio_tools.models.utils"] = types.SimpleNamespace(load_ckpt_state_dict=lambda f: {})
    sys.modules["stable_audio_tools.inference.generation"] = types.SimpleNamespace(generate_diffusion_cond=generate_diffusion_cond)
    import importlib.machinery

    for nom in ("stable_audio_tools",):
        sys.modules[nom].__spec__ = importlib.machinery.ModuleSpec(nom, None)

    import serveur
    import modeles

    chemins = modeles.Chemins(modeles_dir, "Qwen2.5-VL-7B-Instruct-AWQ", "Kokoro-82M", "stable-audio-open-small", "ff_siwis", 300)
    client = TestClient(serveur.creer_app(chemins, 0))
    etat = client.get("/v1/etat").json()
    verifier(etat["modeles"]["son"] == {"installe": True, "nom": "stable-audio-open-small"}, "faux Stable Audio : /v1/etat le voit installé", json.dumps(etat["modeles"]["son"]))
    r = client.post("/v1/son", json={"version": 1, "genre": "ambiance", "prompt": "steady rain", "duree_s": 4.5})
    verifier(r.status_code == 200 and r.headers["content-type"].startswith("audio/wav"), "faux Stable Audio : /v1/son rend un WAV", f"{r.status_code}")
    if r.status_code == 200:
        import soundfile as sf

        donnees, freq = sf.read(io.BytesIO(r.content))
        verifier(freq == 44100 and abs(len(donnees) / freq - 4.5) < 0.01 and donnees.ndim == 2, "durée voulue respectée, stéréo 44,1 kHz", f"{len(donnees) / freq:.2f} s")
        verifier(vu["conditioning"][0]["prompt"] == "steady rain" and vu["t5"].endswith("t5-base") and os.path.isabs(vu["t5"]), "le prompt part, et T5 est cherché en local (hors ligne)", vu["t5"])
    r = client.post("/v1/son", json={"version": 1, "genre": "musique", "prompt": "x", "duree_s": 999})
    if r.status_code == 200:
        import soundfile as sf

        donnees, freq = sf.read(io.BytesIO(r.content))
        verifier(abs(len(donnees) / freq - 524288 / 44100) < 0.01, "une durée trop longue est ramenée à ce que le modèle sait faire", f"{len(donnees) / freq:.1f} s")
    else:
        verifier(False, "durée trop longue", str(r.status_code))
    verifier(client.post("/v1/son", json={"version": 1, "genre": "voix", "prompt": "x"}).status_code == 400, "genre « voix » refusé sur /v1/son")
    verifier(client.post("/v1/son", json={"version": 1, "genre": "bruitage", "prompt": ""}).status_code == 400, "prompt vide refusé")
    for nom in list(sys.modules):
        if nom.startswith("stable_audio_tools"):
            del sys.modules[nom]
    import shutil

    shutil.rmtree(racine, ignore_errors=True)


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--racine", required=True)
    p.add_argument("--sans-lecture", action="store_true")
    p.add_argument("--garder", action="store_true", help="laisse le serveur tourner à la fin")
    a = p.parse_args()
    print("-- plomberie de /v1/son avec un FAUX Stable Audio (le vrai n'est pas installé)")
    essai_son_factice()
    print()
    port = port_libre()
    journal = open(os.path.join(os.environ.get("TEMP", "."), f"shot-sidecar-essai-{port}.log"), "w", encoding="utf-8")
    print("journal du serveur :", journal.name)
    serveur = subprocess.Popen([sys.executable, "-I", os.path.join(ICI, "serveur.py"), "--racine", a.racine, "--port", str(port), "--parent-pid", str(os.getpid())], stdout=journal, stderr=subprocess.STDOUT)
    try:
        for _ in range(100):
            try:
                socket.create_connection(("127.0.0.1", port), timeout=0.2).close()
                break
            except OSError:
                time.sleep(0.2)
        base = vram_mo()
        print(f"\n-- état (mémoire graphique avant tout : {base} Mo)")
        t = time.time()
        code, _, corps = appel(port, "GET", "/v1/etat")
        etat = json.loads(corps)
        verifier(code == 200 and etat["version"] == 1, "GET /v1/etat", f"{time.time() - t:.2f} s : {json.dumps(etat['modeles'])}")

        print("\n-- erreurs")
        code, _, corps = appel(port, "GET", "/v1/inconnu")
        verifier(code == 404 and "erreur" in json.loads(corps), "404 en JSON")
        code, _, corps = appel(port, "POST", "/v1/voix", {"version": 1, "texte": ""})
        verifier(code == 400 and json.loads(corps)["erreur"]["code"] == "invalide", "voix sans texte -> 400 invalide")
        code, _, corps = appel(port, "POST", "/v1/voix", {"version": 1, "texte": "Bonjour.", "voix": "../../x"})
        verifier(code == 400, "nom de voix piégé refusé")
        code, _, corps = appel(port, "POST", "/v1/voix", {"version": 1, "texte": "Bonjour.", "voix": "zz_nulle"})
        verifier(code == 400 and "disponibles" in json.loads(corps)["erreur"]["message"], "voix inconnue -> 400 avec la liste")
        code, _, corps = appel(port, "POST", "/v1/voix", {"version": 7, "texte": "Bonjour."})
        verifier(code == 400, "version du contrat trop récente -> 400")
        if not etat["modeles"]["son"]["installe"]:
            code, _, corps = appel(port, "POST", "/v1/son", {"version": 1, "genre": "bruitage", "prompt": "door slam", "duree_s": 2})
            e = json.loads(corps)["erreur"]
            verifier(code == 501 and e["code"] in ("modele_absent", "non_installe"), "son sans Stable Audio -> 501", e["message"])

        print("\n-- voix (Kokoro)")
        phrases = ["Viens ici.", "Je t'avais dit de ne jamais rouvrir cette porte, et pourtant te voilà, trempé jusqu'aux os, au milieu de la nuit."]
        for i, phrase in enumerate(phrases * 2):
            t = time.time()
            code, ctype, corps = appel(port, "POST", "/v1/voix", {"version": 1, "texte": phrase, "personnage": "MARIE", "voix": "ff_siwis", "langue": "fr"})
            dt = time.time() - t
            if code == 200 and ctype.startswith("audio/wav"):
                import soundfile as sf

                donnees, freq = sf.read(io.BytesIO(corps))
                verifier(len(donnees) > freq * 0.3 and freq == 24000, f"voix {i + 1}{' (1er appel : charge le modèle)' if i == 0 else ''}", f"{len(donnees) / freq:.1f} s de parole en {dt:.2f} s, {len(corps) // 1024} Ko")
                if i == 0:
                    open(os.path.join(os.environ.get("TEMP", "."), "shot-essai-voix.wav"), "wb").write(corps)
            else:
                verifier(False, f"voix {i + 1}", f"{code} {corps[:200]!r}")
        print(f"   mémoire graphique après Kokoro : {vram_mo()} Mo (+{(vram_mo() or 0) - (base or 0)})")

        print("\n-- une demande à la fois : 503 occupé")
        resultats = {}

        def longue():
            resultats["a"] = appel(port, "POST", "/v1/voix", {"version": 1, "texte": " ".join(["Il pleut sur la ville, et personne ne rentre."] * 12), "voix": "ff_siwis"})

        th = threading.Thread(target=longue)
        th.start()
        time.sleep(0.4)
        code, _, corps = appel(port, "POST", "/v1/voix", {"version": 1, "texte": "Moi aussi."})
        verifier(code == 503 and json.loads(corps)["erreur"]["code"] == "occupe", "deuxième demande pendant la première -> 503 occupe", f"code {code}")
        code, _, _ = appel(port, "GET", "/v1/etat")
        verifier(code == 200, "/v1/etat répond pendant qu'une demande travaille")
        th.join()
        verifier(resultats["a"][0] == 200, "la première demande aboutit")

        print("\n-- annulation : fermer la connexion")
        s = socket.create_connection(("127.0.0.1", port))
        corps = json.dumps({"version": 1, "texte": " ".join(["Il pleut sur la ville, et personne ne rentre."] * 43), "voix": "ff_siwis"}).encode()
        s.sendall(b"POST /v1/voix HTTP/1.1\r\nHost: x\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: " + str(len(corps)).encode() + b"\r\n\r\n" + corps)
        time.sleep(0.6)
        s.close()
        t = time.time()
        libre = False
        while time.time() - t < 15:
            code, _, _ = appel(port, "POST", "/v1/voix", {"version": 1, "texte": "Libre."})
            if code == 200:
                libre = True
                break
            time.sleep(0.2)
        verifier(libre, "le serveur est libre peu après la fermeture de la connexion", f"{time.time() - t:.1f} s")
        journal.flush()
        verifier("demande annulée" in open(journal.name, encoding="utf-8").read(), "le journal dit « demande annulée »")

        if not a.sans_lecture and etat["modeles"]["lecture"]["installe"]:
            print("\n-- lecture (Qwen2.5-VL, AWQ sans gptqmodel)")
            plan = {"id": 12, "titre": "Plan 3", "sequence": "1", "cadence": 24, "images": 96,
                    "action": "Une jeune femme referme la porte d'entrée en claquant. Il pleut dehors. Elle monte l'escalier en bois.",
                    "dialogue": "MARIE : Viens ici.\nPAUL : Non.", "notes": "SFX : pas dans l'escalier", "marques": [],
                    "transition_entrante": "Fondu enchaîné, 12 images", "transition_sortante": None, "commentaires": [],
                    "plan_precedent": {"titre": "Plan 2", "action": "Rue sous la pluie."}, "plan_suivant": None}
            sans_image = a.sans_lecture
            for avec_vignette in (False, True):
                corps = {"version": 1, "langue": "fr", "plan": plan}
                if avec_vignette:
                    corps["vignette_png_b64"] = base64.b64encode(vignette_png()).decode()
                avant = vram_mo()
                t = time.time()
                code, _, rep = appel(port, "POST", "/v1/lecture", corps)
                dt = time.time() - t
                if code == 200:
                    liste = json.loads(rep)["indications"]
                    genres = sorted({i["genre"] for i in liste})
                    verifier(len(liste) > 0, f"lecture {'avec' if avec_vignette else 'sans'} vignette", f"{dt:.1f} s, {len(liste)} indications {genres}; mémoire graphique {avant} -> {vram_mo()} Mo")
                    for i in liste:
                        print("      ", i["image"], i["images"], i["genre"], "|", i["description"], "|", i["prompt"], "|", i["texte"])
                else:
                    verifier(False, f"lecture {'avec' if avec_vignette else 'sans'} vignette", f"{code} {rep[:300]!r}")
            print("\n-- lecture annulée en route (fermer la connexion pendant que Qwen écrit)")
            corps = json.dumps({"version": 1, "plan": plan}).encode()
            s = socket.create_connection(("127.0.0.1", port))
            s.sendall(b"POST /v1/lecture HTTP/1.1\r\nHost: x\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: " + str(len(corps)).encode() + b"\r\n\r\n" + corps)
            time.sleep(4)
            s.close()
            t = time.time()
            libre = False
            while time.time() - t < 30:
                code, _, _ = appel(port, "POST", "/v1/voix", {"version": 1, "texte": "Libre."})
                if code != 503:
                    libre = True
                    break
                time.sleep(0.3)
            verifier(libre, "Qwen s'arrête et le serveur se libère après la fermeture de la connexion", f"{time.time() - t:.1f} s")
            print("\n-- voix après Qwen (Qwen est déchargé avant Kokoro)")
            t = time.time()
            code, _, _ = appel(port, "POST", "/v1/voix", {"version": 1, "texte": "Encore moi."})
            verifier(code == 200, "voix après la lecture", f"{time.time() - t:.1f} s (recharge Kokoro) ; mémoire graphique {vram_mo()} Mo")
        print()
    finally:
        if not a.garder:
            serveur.terminate()
            try:
                serveur.wait(10)
            except subprocess.TimeoutExpired:
                serveur.kill()
        journal.close()
    print("TOUT EST OK" if not echecs else f"{len(echecs)} ECHEC(S) : " + ", ".join(echecs))
    return 1 if echecs else 0


if __name__ == "__main__":
    sys.exit(main())
