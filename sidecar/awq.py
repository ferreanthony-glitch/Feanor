"""Qwen2.5-VL AWQ chargé SANS `gptqmodel` ni `autoawq`, avec PyTorch seul.

`transformers` 5 refuse un modèle AWQ tant que `gptqmodel` n'est pas installé (et ce paquet compile des
noyaux CUDA, ce qui est lourd sous Windows). Ce module lit les poids AWQ (4 bits, groupes de 128, « gemm »)
directement dans les fichiers safetensors et les déquantifie couche par couche à chaque passage :
plus lent qu'un noyau CUDA dédié, mais les poids restent en 4 bits dans la carte graphique.

Mémoire (Qwen2.5-VL-7B-Instruct-AWQ, fichiers : 6,9 Go) :
  - les couches du modèle de langage (4 bits)      3,4 Go  -> carte graphique
  - la tête de sortie `lm_head` (fp16)             1,1 Go  -> carte graphique
  - les plongements `embed_tokens` (fp16)          1,1 Go  -> processeur (une simple table de consultation)
  - la tour visuelle (bf16)                        1,4 Go  -> processeur ; passe sur la carte le temps de lire
                                                             l'image, puis revient (le pic reste sous 6 Go)

Aucune dépendance en dehors de torch, transformers, accelerate et safetensors (déjà installés).
"""

import json
import os
import time

import torch
import torch.nn as nn
import torch.nn.functional as F

ORDRE_AWQ = [0, 4, 1, 5, 2, 6, 3, 7]  # ordre inverse de l'empaquetage « gemm » d'AutoAWQ
GROUPE = 128


class AwqLinear(nn.Module):
    """Une couche linéaire AWQ 4 bits. Les entrées sont coupées en deux moitiés : l'octet `q[r, c]` porte
    le poids de la ligne `r` (quartet bas) et celui de la ligne `r + in/2` (quartet haut)."""

    def __init__(self, entrees, sorties, biais, device):
        super().__init__()
        self.entrees, self.sorties = entrees, sorties
        self.register_buffer("q", torch.empty(entrees // 2, sorties, dtype=torch.uint8, device=device))
        groupes = entrees // GROUPE
        self.register_buffer("echelles", torch.empty(groupes, sorties, dtype=torch.float16, device=device))
        self.register_buffer("decalages", torch.empty(groupes, sorties, dtype=torch.float16, device=device))
        if biais:
            self.register_buffer("biais", torch.empty(sorties, dtype=torch.float16, device=device))
        else:
            self.biais = None

    @torch.no_grad()
    def remplir(self, qweight, qzeros, scales, biais=None):
        """Depuis les tenseurs AWQ (sur la carte graphique de préférence)."""
        e, s = self.entrees, self.sorties
        decal = torch.arange(0, 32, 4, device=qweight.device, dtype=torch.int32)

        def deballer(x):
            n = (x.unsqueeze(-1) >> decal) & 15
            n = n.view(x.shape[0], -1, 8)[..., ORDRE_AWQ]
            return n.reshape(x.shape[0], -1)

        poids = deballer(qweight).to(torch.uint8)  # [e, s]
        zeros = deballer(qzeros).to(torch.float32)  # [groupes, s]
        moitie = e // 2
        self.q.copy_(poids[:moitie] | (poids[moitie:] << 4))
        sc = scales.to(torch.float32)
        self.echelles.copy_(sc.to(torch.float16))
        self.decalages.copy_((-zeros * sc).to(torch.float16))
        if biais is not None and self.biais is not None:
            self.biais.copy_(biais.to(torch.float16))

    def _moitie(self, x, q, debut_groupe):
        g = q.shape[0] // GROUPE
        sl = slice(debut_groupe, debut_groupe + g)
        w = q.to(x.dtype).view(g, GROUPE, self.sorties)
        w = torch.addcmul(self.decalages[sl].unsqueeze(1), w, self.echelles[sl].unsqueeze(1))
        return x @ w.view(q.shape[0], self.sorties)

    def forward(self, x):
        forme = x.shape
        x2 = x.reshape(-1, self.entrees)
        h = self.entrees // 2
        y = self._moitie(x2[:, :h], self.q & 15, 0)
        y = y + self._moitie(x2[:, h:], self.q >> 4, h // GROUPE)
        if self.biais is not None:
            y = y + self.biais
        return y.view(*forme[:-1], self.sorties)


class EmbeddingProcesseur(nn.Module):
    """Table de plongements restée dans la mémoire de l'ordinateur ; le résultat part sur la carte."""

    def __init__(self, poids, destination):
        super().__init__()
        self.weight = nn.Parameter(poids, requires_grad=False)
        self.destination = destination

    def forward(self, ids):
        return F.embedding(ids.to("cpu"), self.weight).to(self.destination)


def _nom_modele(nom):
    if nom.startswith("model.layers.") or nom.startswith("model.embed_tokens.") or nom.startswith("model.norm."):
        return "model.language_model." + nom[len("model."):]
    if nom.startswith("visual."):
        return "model." + nom
    return nom


def _module(modele, chemin):
    m = modele
    for p in chemin.split("."):
        m = getattr(m, p)
    return m


def charger_qwen_awq(dossier, device="cuda", rapport=None):
    """Rend (modèle, durée_s). `rapport(texte)` reçoit l'avancement."""
    from accelerate.utils import set_module_tensor_to_device
    from safetensors import safe_open
    from transformers import AutoConfig, Qwen2_5_VLForConditionalGeneration

    debut = time.time()
    dit = rapport or (lambda t: None)
    cfg = AutoConfig.from_pretrained(dossier)
    if hasattr(cfg, "quantization_config"):
        cfg.quantization_config = None
    with torch.device("meta"):
        modele = Qwen2_5_VLForConditionalGeneration._from_config(cfg, dtype=torch.float16)

    with open(os.path.join(dossier, "model.safetensors.index.json"), encoding="utf-8") as f:
        index = json.load(f)["weight_map"]
    fichiers = {}
    for fic in sorted(set(index.values())):
        fichiers[fic] = safe_open(os.path.join(dossier, fic), framework="pt", device="cpu")

    def tenseur(nom):
        return fichiers[index[nom]].get_tensor(nom)

    # Les couches linéaires quantifiées : celles qui ont un `qweight`.
    quantifiees = sorted({n[: -len(".qweight")] for n in index if n.endswith(".qweight")})
    for i, base in enumerate(quantifiees):
        cible = _nom_modele(base)
        parent_chemin, _, feuille = cible.rpartition(".")
        parent = _module(modele, parent_chemin)
        ancien = getattr(parent, feuille)
        biais_nom = base + ".bias"
        couche = AwqLinear(ancien.in_features, ancien.out_features, biais_nom in index, device)
        qw = tenseur(base + ".qweight").to(device)
        qz = tenseur(base + ".qzeros").to(device)
        sc = tenseur(base + ".scales").to(device)
        bi = tenseur(biais_nom).to(device) if biais_nom in index else None
        couche.remplir(qw, qz, sc, bi)
        setattr(parent, feuille, couche)
        if i % 40 == 0:
            dit(f"couches {i}/{len(quantifiees)}")

    ensemble = set(quantifiees)
    reste = [n for n in index if not n.endswith((".qweight", ".qzeros", ".scales")) and not (n.endswith(".bias") and n[:-5] in ensemble)]
    for nom in reste:
        cible = _nom_modele(nom)
        t = tenseur(nom)
        if cible.startswith("model.visual."):
            set_module_tensor_to_device(modele, cible, "cpu", value=t.to(torch.bfloat16))
        elif cible.startswith("model.language_model.embed_tokens."):
            set_module_tensor_to_device(modele, cible, "cpu", value=t.to(torch.float16))
        else:
            set_module_tensor_to_device(modele, cible, device, value=t.to(torch.float16))

    # Tampons non enregistrés dans le fichier (fréquences des rotations) : on refait les modules.
    lm = modele.model.language_model
    lm.rotary_emb = type(lm.rotary_emb)(config=lm.rotary_emb.config).to(device)
    vis = modele.model.visual
    vis.rotary_pos_emb = type(vis.rotary_pos_emb)(vis.rotary_pos_emb.config)  # reste sur le processeur avec la tour

    # Les plongements restent sur le processeur.
    lm.embed_tokens = EmbeddingProcesseur(lm.embed_tokens.weight.data, device)

    # La tour visuelle vient sur la carte le temps de lire l'image.
    def _vers_carte(m, args, kwargs=None):
        m.to(device)

    def _vers_processeur(m, args, sortie):
        m.to("cpu")
        if torch.cuda.is_available():
            torch.cuda.empty_cache()
        return sortie

    vis.register_forward_pre_hook(_vers_carte)
    vis.register_forward_hook(_vers_processeur)

    meta = [n for n, p in list(modele.named_parameters()) + list(modele.named_buffers()) if p.device.type == "meta"]
    if meta:
        raise RuntimeError("poids non chargés : " + ", ".join(meta[:5]))
    # `generate` pose ses tenseurs sur `modele.device`, qui est celui du premier paramètre (la tour visuelle,
    # sur le processeur) : on dit que le modèle est sur la carte.
    classe = type("Qwen2_5_VLAwq", (type(modele),), {"device": property(lambda self: torch.device(device))})
    modele.__class__ = classe
    # Avec transformers 5.20.1, les sorties de la tour visuelle pré-calculées par `generate` arrivent à `forward`
    # sous forme de simples dictionnaires : on les remet en `BaseModelOutputWithPooling`.
    from transformers.modeling_outputs import BaseModelOutputWithPooling

    import functools

    forward_origine = modele.forward

    @functools.wraps(forward_origine)
    def forward(*args, **kwargs):
        mm = kwargs.get("mm_encoder_outputs")
        if mm:
            kwargs["mm_encoder_outputs"] = {n: (BaseModelOutputWithPooling(**v) if isinstance(v, dict) and not hasattr(v, "pooler_output") else v) for n, v in mm.items()}
        return forward_origine(*args, **kwargs)

    modele.forward = forward
    modele.eval()
    dit("prêt")
    return modele, time.time() - debut
