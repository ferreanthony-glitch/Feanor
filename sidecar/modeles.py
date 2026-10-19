"""Les trois modèles du serveur local et leur gestionnaire de mémoire graphique.

- lecture : Qwen2.5-VL-7B-Instruct-AWQ (par `awq.py`, sans gptqmodel), lit un plan et rend des indications ;
- voix    : Kokoro-82M, une réplique -> un WAV 24 kHz ;
- son     : Stable Audio Open (Small), un prompt -> un WAV 44,1 kHz.

Un seul gros modèle reste dans la carte à la fois : `Gestionnaire.exiger(role)` décharge celui qui gêne avant
de charger l'autre. Kokoro (0,4 Go) peut rester avec Stable Audio ; il part avant Qwen.
Rien ne passe par le réseau : `HF_HUB_OFFLINE=1` est posé par `serveur.py` avant tout import.
"""

import gc
import importlib.util
import io
import json
import os
import re
import threading
import time


class Annule(Exception):
    """La demande a été annulée (la connexion s'est fermée)."""


class ErreurApi(Exception):
    """Une erreur du contrat (`{"erreur": {"code", "message"}}`)."""

    def __init__(self, statut, code, message):
        super().__init__(message)
        self.statut, self.code, self.message = statut, code, message


def _torch():
    import torch

    return torch


def liberer_cuda():
    gc.collect()
    try:
        t = _torch()
        if t.cuda.is_available():
            t.cuda.empty_cache()
    except Exception:
        pass


def vram_utilisee_mo():
    """Mémoire graphique prise par ce processus (Mo), ou None sans carte."""
    try:
        t = _torch()
        if t.cuda.is_available():
            return round(t.cuda.memory_reserved() / 2**20)
    except Exception:
        pass
    return None


# ----------------------------------------------------------------------------------- où sont les modèles


class Chemins:
    """Les dossiers des modèles, d'après la configuration."""

    def __init__(self, dossier, lecture, voix, son, voix_defaut, max_tokens, t5="t5-base"):
        self.dossier = dossier  # dossier qui contient un sous-dossier par modèle (peut être None)
        self.lecture, self.voix, self.son = lecture, voix, son
        self.voix_defaut = voix_defaut
        self.max_tokens = max_tokens
        self.t5 = t5  # l'encodeur de texte de Stable Audio (google-t5/t5-base), à côté de lui

    def _sous(self, nom):
        return os.path.join(self.dossier, nom) if self.dossier and nom else None

    def dossier_lecture(self):
        return self._sous(self.lecture)

    def dossier_voix(self):
        return self._sous(self.voix)

    def dossier_son(self):
        return self._sous(self.son)

    def dossier_t5(self):
        return self._sous(self.t5)

    # --- ce qui est installé (sans rien charger)

    def etat_lecture(self):
        d = self.dossier_lecture()
        if d and os.path.isfile(os.path.join(d, "config.json")) and os.path.isfile(os.path.join(d, "model.safetensors.index.json")):
            return {"installe": True, "nom": os.path.basename(d)}
        return {"installe": False}

    def etat_voix(self):
        d = self.dossier_voix()
        if d and os.path.isfile(os.path.join(d, "kokoro-v1_0.pth")) and os.path.isfile(os.path.join(d, "config.json")):
            return {"installe": True, "nom": "Kokoro-82M"}
        return {"installe": False}

    def etat_son(self):
        d = self.dossier_son()
        if not d or not os.path.isdir(d):
            return {"installe": False}
        outils = os.path.isfile(os.path.join(d, "model_config.json")) and any(os.path.isfile(os.path.join(d, f)) for f in ("model.safetensors", "model.ckpt"))
        diffusers = os.path.isfile(os.path.join(d, "model_index.json"))
        t5 = self.dossier_t5()
        if outils and importlib.util.find_spec("stable_audio_tools") and t5 and os.path.isfile(os.path.join(t5, "config.json")):
            return {"installe": True, "nom": os.path.basename(d)}
        if diffusers and importlib.util.find_spec("diffusers"):
            return {"installe": True, "nom": os.path.basename(d)}
        return {"installe": False}

    def raison_son_absent(self):
        d = self.dossier_son()
        if not d or not os.path.isdir(d):
            return "Stable Audio Open n'est pas installé : le dossier du modèle est absent (voir docs/GENERATION_SON.md)."
        if os.path.isfile(os.path.join(d, "model_config.json")) and not importlib.util.find_spec("stable_audio_tools"):
            return "Stable Audio Open est téléchargé mais la bibliothèque Python « stable-audio-tools » manque."
        t5 = self.dossier_t5()
        if os.path.isfile(os.path.join(d, "model_config.json")) and not (t5 and os.path.isfile(os.path.join(t5, "config.json"))):
            return f"Stable Audio Open est téléchargé mais son encodeur de texte (google-t5/t5-base) manque : il doit être dans le dossier « {self.t5} » à côté des modèles."
        if os.path.isfile(os.path.join(d, "model_index.json")) and not importlib.util.find_spec("diffusers"):
            return "Stable Audio Open est téléchargé mais la bibliothèque Python « diffusers » manque."
        return "Stable Audio Open est incomplet : model_config.json et model.safetensors sont attendus dans son dossier."


# ----------------------------------------------------------------------------------------------- Kokoro

LANGUES_KOKORO = {"a": "a", "b": "b", "e": "e", "f": "f", "h": "h", "i": "i", "j": "j", "p": "p", "z": "z"}


class VoixKokoro:
    def __init__(self, dossier, device):
        t = _torch()
        from kokoro import KModel

        self.dossier = dossier
        self.device = device
        self.modele = KModel(config=os.path.join(dossier, "config.json"), model=os.path.join(dossier, "kokoro-v1_0.pth")).to(device).eval()
        self.pipelines = {}
        self.voix = {}
        self._t = t

    def voix_disponibles(self):
        d = os.path.join(self.dossier, "voices")
        return sorted(f[:-3] for f in os.listdir(d) if f.endswith(".pt")) if os.path.isdir(d) else []

    def _pipeline(self, code):
        from kokoro import KPipeline

        if code not in self.pipelines:
            self.pipelines[code] = KPipeline(lang_code=code, model=self.modele, repo_id="hexgrad/Kokoro-82M")
        return self.pipelines[code]

    def _voix(self, nom):
        if nom not in self.voix:
            self.voix[nom] = self._t.load(os.path.join(self.dossier, "voices", nom + ".pt"), weights_only=True)
        return self.voix[nom]

    def synthetiser(self, texte, nom_voix, annule):
        """Rend (échantillons float32 numpy mono, fréquence)."""
        import numpy as np

        code = LANGUES_KOKORO.get(nom_voix[:1], "f")
        pipe = self._pipeline(code)
        voix = self._voix(nom_voix)
        morceaux = []
        for _, _, audio in pipe(texte, voice=voix):
            if annule.is_set():
                raise Annule()
            if audio is not None:
                morceaux.append(self._t.as_tensor(audio).detach().float().cpu())
        if not morceaux:
            raise ErreurApi(422, "invalide", "Kokoro n'a rien pu dire de ce texte.")
        return self._t.cat(morceaux).numpy().astype(np.float32), 24000


def wav_octets(echantillons, frequence):
    import soundfile as sf

    tampon = io.BytesIO()
    sf.write(tampon, echantillons, frequence, format="WAV", subtype="PCM_16")
    return tampon.getvalue()


# ---------------------------------------------------------------------------------------------- Qwen

# Le serveur écrit SES consignes (le champ « consignes » de SHOT est permis d'être ignoré, contrat §6) : un
# modèle de 7 Go quantifié écrit ~4 jetons/s sur cette carte, donc on lui demande le moins possible —
# une ligne par son, sans JSON, et SANS les voix, que le serveur tire lui-même des lignes de dialogue
# (jamais de réplique inventée ni déformée, et la moitié des jetons en moins).
CONSIGNES = (
    "You are a sound designer reading the storyboard of an animated film, one shot at a time, to place TEMPORARY sounds in the animatic.\n"
    "List the sounds the shot needs: sound effects (bruitage), ambiences (ambiance), and a music (musique) only if the texts ask for one. "
    "Voices are added elsewhere: never list dialogue. Read the texts first; use the thumbnail (if any) to understand the place and the action. "
    "Propose few, useful sounds (0 to 5). No sound if the shot calls for none.\n"
    "One sound per line, exactly in this format and nothing else:\n"
    "genre | first frame | duration in frames | short description in FRENCH | short ENGLISH prompt for a sound generator\n"
    "Frames count from 0 inside the shot. Sound effects last 1 to 3 seconds. Ambiences and music last until the end of the shot. "
    "If the shot needs no sound, answer only the word: none (never put \"none\" inside a line, and write no line for a sound that is absent).\n"
    "Example for a shot of 120 frames at 24 fps (use its form only, never its content):\n"
    "bruitage | 10 | 36 | Pas dans le gravier | slow footsteps on gravel\n"
    "ambiance | 0 | 120 | Vent dans les arbres | wind blowing through trees"
)


def objets_json(texte):
    """Les objets JSON complets d'un texte de modèle (même coupé en route) qui ont un « genre »."""
    dec = json.JSONDecoder()
    trouves = []
    i = 0
    while True:
        j = texte.find("{", i)
        if j < 0:
            break
        try:
            obj, fin = dec.raw_decode(texte, j)
        except ValueError:
            i = j + 1
            continue
        if isinstance(obj, dict) and "indications" in obj and isinstance(obj["indications"], list):
            trouves.extend(o for o in obj["indications"] if isinstance(o, dict))
            i = fin
        elif isinstance(obj, dict) and "genre" in obj:
            trouves.append(obj)
            i = fin
        else:
            i = j + 1
    return trouves


GENRES = {"bruitage", "ambiance", "musique", "voix"}


def nettoyer_indications(objets, images):
    """Garde les champs du contrat, avec les bons types."""
    sortie = []
    for o in objets[:200]:
        genre = str(o.get("genre", "")).strip().lower()
        if genre == "sfx":
            genre = "bruitage"
        if genre not in GENRES:
            continue

        def entier(cle, defaut):
            try:
                return max(0, int(round(float(o.get(cle, defaut)))))
            except (TypeError, ValueError):
                return defaut

        debut = min(entier("image", 0), max(images - 1, 0))
        duree = max(1, entier("images", max(images - debut, 1)))
        texte = lambda cle, m: str(o.get(cle, "") or "").strip()[:m]
        sortie.append({
            "image": debut, "images": duree, "genre": genre,
            "description": texte("description", 400), "prompt": texte("prompt", 600),
            "texte": texte("texte", 1000), "personnage": texte("personnage", 100),
        })
    return sortie


def texte_du_plan(plan):
    """Le plan en texte lisible pour le modèle."""
    images = int(plan.get("images") or 1)
    cadence = plan.get("cadence") or 24
    lignes = [f"Shot \"{plan.get('titre', '')}\": {images} frames at {cadence} fps ({images / max(cadence, 1):.1f} s)."]
    if plan.get("action"):
        lignes.append("Action: " + str(plan["action"]).strip())
    if plan.get("notes"):
        lignes.append("Notes: " + str(plan["notes"]).strip())
    if plan.get("dialogue"):
        lignes.append("Dialogue (context only, voices are handled elsewhere): " + " / ".join(str(plan["dialogue"]).strip().splitlines()))
    for m in plan.get("marques") or []:
        if isinstance(m, dict):
            texte = " ".join(str(m.get(k) or "").strip() for k in ("action", "notes")).strip()
            if texte:
                lignes.append(f"At frame {m.get('image', 0)}: {texte}")
    if plan.get("transition_entrante"):
        lignes.append("Transition in: " + str(plan["transition_entrante"]))
    if plan.get("transition_sortante"):
        lignes.append("Transition out: " + str(plan["transition_sortante"]))
    for c in plan.get("commentaires") or []:
        lignes.append("Director's comment: " + str(c))
    for cle, nom in (("plan_precedent", "Previous shot"), ("plan_suivant", "Next shot")):
        v = plan.get(cle)
        if isinstance(v, dict) and v.get("action"):
            lignes.append(f"{nom}: {str(v['action']).strip()[:200]}")
    return "\n".join(lignes)[:4000]


def sons_depuis_lignes(texte, images, cadence=24):
    """Les lignes « genre | image | images | description | prompt » d'une réponse. Une ligne bancale est écartée.
    Un bruitage dure 3 s au plus (le modèle l'oublie parfois) ; une ambiance ou une musique, 30 s."""
    sortie = []
    for ligne in texte.splitlines():
        ligne = re.sub(r"^\s*(?:[-*•]|\d+[.)])\s*", "", ligne).strip()
        champs = [c.strip() for c in ligne.split("|")]
        if len(champs) < 4:
            continue
        genre = champs[0].lower().strip("`*\"' ")
        if genre == "sfx":
            genre = "bruitage"
        if genre not in ("bruitage", "ambiance", "musique"):
            continue
        nombres = [re.search(r"\d+", c) for c in champs[1:3]]
        debut = int(nombres[0].group()) if nombres[0] else 0
        duree = int(nombres[1].group()) if nombres[1] else None
        debut = min(debut, max(images - 1, 0))
        if duree is None or duree < 1:
            duree = max(images - debut, 1) if genre != "bruitage" else min(2 * cadence, max(images - debut, 1))
        duree = min(duree, (3 if genre == "bruitage" else 30) * cadence)
        description = champs[3]
        prompt = champs[4] if len(champs) > 4 else ""
        vide = {"", "none", "aucun", "aucune", "n/a", "-"}
        if description.lower() in vide or prompt.lower() in vide and not description:
            continue
        if prompt.lower() in vide:
            prompt = description
        sortie.append({"image": debut, "images": duree, "genre": genre, "description": description[:400], "prompt": (prompt or description)[:600], "texte": "", "personnage": ""})
    return sortie


def _sans_parentheses(s):
    sortie, prof = [], 0
    for c in s:
        if c in "([":
            prof += 1
        elif c in ")]" and prof > 0:
            prof -= 1
        elif prof == 0:
            sortie.append(c)
    return "".join(sortie)


def separer_replique(ligne):
    """« MARIE (off) : Viens. » -> ("MARIE", "Viens.") ; « - Viens. » -> ("", "Viens.") (comme lecturerules.rs)."""
    ligne = ligne.strip().lstrip("-–—•* ").strip()
    for sep in (":", "："):
        i = ligne.find(sep)
        if i > 0:
            prefixe, reste = ligne[:i], ligne[i + 1:].strip()
            sans = _sans_parentheses(prefixe)
            mots = sans.split()
            if reste and 0 < len(mots) <= 3 and not any(c.isdigit() or c in ".!?«»\"" for c in sans):
                lettres = [c for c in sans if c.isalpha()]
                if all(c.isupper() for c in lettres) or all(m[0].isupper() for m in mots):
                    return " ".join(mots), reste.strip().strip("«»\"“” ").strip()
            break
    return "", ligne.strip().strip("«»\"“” ").strip()


def voix_depuis_dialogue(plan):
    """Une voix par ligne de dialogue du plan et de ses marques, l'une après l'autre dans le temps."""
    images = int(plan.get("images") or 1)
    cadence = int(plan.get("cadence") or 24)
    derniere = max(images - 1, 0)
    sources = [(0, str(plan.get("dialogue") or ""))]
    for m in plan.get("marques") or []:
        if isinstance(m, dict):
            try:
                sources.append((min(int(m.get("image", 0)), derniere), str(m.get("dialogue") or "")))
            except (TypeError, ValueError):
                pass
    sortie = []
    for debut, dialogue in sources:
        au = debut
        for ligne in (l.strip() for l in dialogue.splitlines()):
            if not ligne:
                continue
            personnage, texte = separer_replique(ligne)
            if not any(c.isalnum() for c in texte):
                continue
            texte = texte[:600]
            duree = max(1, round(max(len(texte) / 14.0, 1.0) * cadence))
            extrait = texte[:70] + ("…" if len(texte) > 70 else "")
            description = f"{personnage} : « {extrait} »" if personnage else f"Voix : « {extrait} »"
            sortie.append({"image": min(au, derniere), "images": duree, "genre": "voix", "description": description, "prompt": "", "texte": texte, "personnage": personnage[:100]})
            au += duree + max(1, round(0.4 * cadence))
    return sortie


class LecteurQwen:
    def __init__(self, dossier, device, max_tokens):
        from awq import charger_qwen_awq
        from transformers import AutoProcessor

        self.device = device
        self.max_tokens = max_tokens
        self.modele, self.duree_chargement = charger_qwen_awq(dossier, device)
        self.processeur = AutoProcessor.from_pretrained(dossier)

    def lire(self, plan, vignette_png, annule):
        """Rend (texte de la réponse, jetons, secondes)."""
        t = _torch()
        from transformers import StoppingCriteria, StoppingCriteriaList

        question = texte_du_plan(plan) + "\nList the sounds, one per line."
        contenu = []
        image = None
        if vignette_png:
            from PIL import Image

            image = Image.open(io.BytesIO(vignette_png)).convert("RGB")
            contenu.append({"type": "image"})
        contenu.append({"type": "text", "text": question})
        messages = [{"role": "system", "content": CONSIGNES}, {"role": "user", "content": contenu}]
        invite = self.processeur.apply_chat_template(messages, tokenize=False, add_generation_prompt=True)
        entrees = self.processeur(text=[invite], images=[image] if image is not None else None, return_tensors="pt").to(self.device)

        class Arret(StoppingCriteria):
            def __call__(self, ids, scores, **kw):
                return annule.is_set()

        debut = time.time()
        with t.inference_mode():
            sortie = self.modele.generate(**entrees, max_new_tokens=self.max_tokens, do_sample=False, stopping_criteria=StoppingCriteriaList([Arret()]))
        if annule.is_set():
            raise Annule()
        nouveaux = sortie[:, entrees["input_ids"].shape[1]:]
        texte = self.processeur.batch_decode(nouveaux, skip_special_tokens=True)[0]
        return texte, int(nouveaux.shape[1]), time.time() - debut


# --------------------------------------------------------------------------------------- Stable Audio


class SonStableAudio:
    """Stable Audio Open par `stable-audio-tools` (Small, 8 étapes « pingpong ») ou `diffusers` (1.0).
    NON ESSAYÉ ici : les poids ne sont pas téléchargés (licence à accepter par l'utilisateur)."""

    def __init__(self, dossier, device, dossier_t5=None):
        t = _torch()
        self.device = device
        self.dossier = dossier
        self._t = t
        if os.path.isfile(os.path.join(dossier, "model_config.json")):
            from stable_audio_tools.models.factory import create_model_from_config
            from stable_audio_tools.models.utils import load_ckpt_state_dict

            with open(os.path.join(dossier, "model_config.json"), encoding="utf-8") as f:
                self.config = json.load(f)

            # Le texte est encodé par T5 (« t5-base » dans la configuration, un nom du Hugging Face Hub) :
            # hors ligne, on le remplace par son dossier local.
            def vers_local(noeud):
                if isinstance(noeud, dict):
                    for cle, valeur in noeud.items():
                        if cle == "t5_model_name" and dossier_t5:
                            noeud[cle] = dossier_t5
                        else:
                            vers_local(valeur)
                elif isinstance(noeud, list):
                    for x in noeud:
                        vers_local(x)

            vers_local(self.config)
            modele = create_model_from_config(self.config)
            fichier = next(os.path.join(dossier, n) for n in ("model.safetensors", "model.ckpt") if os.path.isfile(os.path.join(dossier, n)))
            modele.load_state_dict(load_ckpt_state_dict(fichier))
            self.modele = modele.to(device).eval().requires_grad_(False)
            self.mode = "outils"
            self.frequence = int(self.config.get("sample_rate", 44100))
            self.taille = int(self.config.get("sample_size", 524288))
        else:
            from diffusers import StableAudioPipeline

            self.modele = StableAudioPipeline.from_pretrained(dossier, torch_dtype=t.float16).to(device)
            self.mode = "diffusers"
            self.frequence = int(self.modele.vae.sampling_rate)
            self.taille = int(self.frequence * 47)

    @property
    def duree_max(self):
        return self.taille / self.frequence

    def generer(self, prompt, secondes, annule):
        """Rend (échantillons numpy [n] ou [n, canaux] float32, fréquence)."""
        import numpy as np

        t = self._t
        secondes = max(0.5, min(float(secondes), self.duree_max))
        if self.mode == "outils":
            from stable_audio_tools.inference.generation import generate_diffusion_cond

            conditionnement = [{"prompt": prompt, "seconds_start": 0, "seconds_total": secondes}]
            with t.inference_mode():
                sortie = generate_diffusion_cond(
                    self.modele, steps=8, cfg_scale=1.0, conditioning=conditionnement, sample_size=self.taille,
                    sampler_type="pingpong", device=self.device,
                )
            audio = sortie[0].to(t.float32).cpu()
        else:
            with t.inference_mode():
                sortie = self.modele(prompt, negative_prompt="Low quality.", audio_end_in_s=secondes, num_inference_steps=100, num_waveforms_per_prompt=1)
            audio = t.as_tensor(sortie.audios[0]).to(t.float32).cpu()
        if annule.is_set():
            raise Annule()
        audio = audio[:, : int(secondes * self.frequence)]
        crete = float(audio.abs().max())
        if crete > 1.0:
            audio = audio / crete
        return audio.transpose(0, 1).numpy().astype(np.float32), self.frequence


# --------------------------------------------------------------------------------------- gestionnaire


class Gestionnaire:
    """Charge à la demande, décharge ce qui gêne. À n'appeler que sous le verrou « une demande à la fois »."""

    LEGERS = {"voix", "son"}  # peuvent rester ensemble ; « lecture » est exclusive

    def __init__(self, chemins):
        self.chemins = chemins
        self.charges = {}
        self.derniere_activite = time.time()
        self.mesures = {}  # role -> secondes de chargement (pour les journaux)

    def device(self):
        return "cuda" if _torch().cuda.is_available() else "cpu"

    def decharger(self, role):
        if role in self.charges:
            del self.charges[role]
            liberer_cuda()
            journal(f"modèle « {role} » déchargé (mémoire graphique : {vram_utilisee_mo()} Mo)")

    def tout_decharger(self):
        for role in list(self.charges):
            self.decharger(role)

    def exiger(self, role):
        self.derniere_activite = time.time()
        if role in self.charges:
            return self.charges[role]
        for autre in list(self.charges):
            if not (role in self.LEGERS and autre in self.LEGERS):
                self.decharger(autre)
        debut = time.time()
        try:
            if role == "lecture":
                if _torch().cuda.is_available() is False:
                    raise ErreurApi(503, "indisponible", "La lecture par Qwen demande une carte graphique NVIDIA (CUDA) : aucune n'est utilisable.")
                objet = LecteurQwen(self.chemins.dossier_lecture(), "cuda", self.chemins.max_tokens)
            elif role == "voix":
                objet = VoixKokoro(self.chemins.dossier_voix(), self.device())
            else:
                objet = SonStableAudio(self.chemins.dossier_son(), self.device(), self.chemins.dossier_t5())
        except _torch().cuda.OutOfMemoryError:
            self.tout_decharger()
            raise ErreurApi(503, "indisponible", "Plus assez de mémoire graphique pour charger ce modèle. Ferme les programmes qui l'utilisent et réessaie.")
        self.charges[role] = objet
        self.mesures[role] = time.time() - debut
        journal(f"modèle « {role} » chargé en {self.mesures[role]:.1f} s (mémoire graphique : {vram_utilisee_mo()} Mo)")
        return objet


def journal(texte):
    print(time.strftime("%H:%M:%S"), texte, flush=True)
