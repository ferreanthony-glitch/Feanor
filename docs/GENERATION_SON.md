# Génération sonore à partir du storyboard

Branche `generation-son` (phase 1 : tout ce qui se passe du côté de SHOT), puis branche `sidecar-son`
(**phase 2 : le serveur Python local est écrit, branché à SHOT et essayé de bout en bout**, voir §8). Aucun
téléchargement n'a été fait pour l'écrire : le Python, le venv et les modèles de `SHOT-modeles` étaient déjà
installés avec l'accord d'Anthony ; le serveur n'a aucun accès au réseau (`HF_HUB_OFFLINE=1`).

Ce document sert à quatre choses : dire ce que SHOT fait, **fixer le contrat** du serveur local (§6), dire
**comment le lancer et ce qui est installé** (§8), et ce qui manque encore (§8.5).

---------------------------------------------------------------------------------------------------

## 1. Ce que fait la fonction

SHOT lit le storyboard et **propose des sons témoins**, de qualité animatique : des bruitages, des
ambiances, une musique d'intention, des voix pour les dialogues. Le réalisateur les écoute et les accepte, les
régénère ou les supprime. Rien n'entre dans le montage sans son accord.

- Panneau **Suggestions sonores** (menu **Labo** > Suggestions sonores…, commande
  `vue.suggestions_sonores`), fermé par défaut, ouvert dans la colonne de droite.
- **Lire le storyboard** (`suggestion.lire`) sur tout le film, la séquence courante, le plan courant ou une
  plage de plans. La liste montre pour chaque proposition : l'icône de la sorte, le plan et le moment dans le plan,
  la description, l'état (proposée, en génération, prête, acceptée, rejetée).
- Par proposition (clic, clic droit, ou double-clic = écouter / générer) : **Générer / Régénérer**, **Écouter**,
  **Accepter**, **Rejeter** (et Reprendre), **Modifier**, **Supprimer**. Pour toute la liste : **Tout
  générer**, **Tout accepter**, **Supprimer les rejetées**. **Annuler** arrête ce qui travaille.
- **Accepter** copie le son dans le projet (comme un son importé) et le pose à l'image voulue sur la piste
  **« Sons proposés »** (créée au besoin ; « Sons proposés 2 » si la première est occupée à cet endroit ; une piste
  verrouillée ou celle d'un autre nom n'est jamais prise). Une étape d'annulation : « Accepter un son proposé ».
  Annuler la pose rend la proposition « prête », la rétablir la rend « acceptée ».
- Une relecture du storyboard **ne refait pas** ce que le réalisateur a déjà vu : une proposition connue (même sorte,
  même description, ou même réplique) n'est pas rajoutée, **rejetée comprise** ; une proposition intacte que le
  texte du plan ne demande plus est retirée ; une proposition modifiée à la main ou déjà générée reste.

### Les trois rôles

Chaque rôle a son fournisseur, choisi dans **Fichier > Préférences > Génération sonore** :

| Rôle | Règles | Local (serveur de cet ordinateur) | Service en ligne |
|---|---|---|---|
| 1. Lecture du storyboard → indications | **par défaut**, sans modèle | Qwen2.5-VL | Claude (API) |
| 2. Bruitages et ambiances | — | Stable Audio Open (Small ou 1.0) | ElevenLabs Sound Effects, ou API de Stable Audio |
| 3. Voix témoin des dialogues | — | Kokoro-82M | ElevenLabs Text to Speech |

### Ce qui marche maintenant, sans aucun modèle

La **lecture par règles** (`lecturerules.rs`) est le choix d'origine du rôle 1. Elle lit l'action, le dialogue et
les notes du plan (et ceux de ses images de storyboard) :

- **Chaque ligne de dialogue devient une voix.** « MARIE : Viens ici. » donne le personnage et la réplique ; une ligne
  sans nom (« - Non. ») est une réplique sans personnage. Les répliques d'un plan se suivent dans le temps (une seconde
  au moins chacune, 14 caractères par seconde, 0,4 s de pause).
- **Des mots-clés** de l'action et des notes (jamais du dialogue : un personnage qui dit « il pleut » ne fait pas
  tomber la pluie) donnent des bruitages (porte, pas, explosion, coup de feu, voiture, klaxon, téléphone, sonnette, verre
  brisé, coup, cri, tonnerre, gouttes, plouf, applaudissements, épée, cloche, alarme, battements de cœur, respiration,
  rires), des ambiances (pluie, vent, foule, café, campagne, forêt, mer, ville, usine, cour d'école, cours d'eau, feu,
  grotte, gare) et une musique d'intention (piano, violon, guitare, fanfare, berceuse, musique en général). La recherche
  ignore la casse, les accents et la ponctuation, ne cherche que des mots entiers (ou un début de mot : « explos* »), et
  écarte un mot nié (« sans musique », « pas de pluie », « aucun vent »).
- **Les consignes écrites** sont reprises : une ligne « SFX : porte qui grince, tic-tac de pendule » (ou « Son : »,
  « Bruitage : », « Ambiance : », « Musique : ») donne une proposition par élément. Ceux que les règles connaissent
  reçoivent leur description et leur prompt anglais soignés ; les autres sont repris tels quels (le texte sert de
  description et de prompt, sans traduction : à corriger avec « Modifier »).
- Les ambiances et la musique couvrent le reste du plan (au plus 30 s), un bruitage dure ce que dit sa règle ; une
  seule ambiance par règle et par plan, une seule musique par plan, 40 propositions au plus par plan.

Les règles proposent ; elles ne **génèrent** pas de son. Générer demande le rôle 2 (et le rôle 3 pour les voix) :
si le modèle du rôle n'est pas installé (Stable Audio Open, pour l'instant) ou qu'aucun service n'est branché,
« Générer » répond « non installé » ou « connexion en ligne à brancher ». Les voix (Kokoro) marchent déjà ; les
bruitages attendent Stable Audio (§8.5).

---------------------------------------------------------------------------------------------------

## 2. Où les choses sont rangées

| Quoi | Où | Remarques |
|---|---|---|
| Les propositions | `sons_proposes.json` à côté du manifeste du projet | Format `shot-sons-proposes`, version 1, **sans changer `VERSION`** du projet. Mêmes protections que `commentaires.json` : un fichier illisible ou plus récent ouvre le projet sans propositions, avec un avertissement, et n'est **jamais effacé** (mis de côté en `.illisible` si une proposition neuve doit s'écrire) ; écriture atomique par `prepare_save` / `SaveJob`. |
| La confirmation d'envoi en ligne | dans ce même fichier (`envoi_en_ligne_confirme`) | Une fois par projet. |
| Les sons générés, avant acceptation | `%LOCALAPPDATA%\SHOT\sons_generes\` | Le chemin est gardé dans la proposition. Un fichier disparu (cache vidé, autre ordinateur) redonne une proposition à générer. Le cache n'est pas nettoyé automatiquement (à faire). |
| Les sons acceptés | `sons/` du projet | Copiés tels quels, comme un son importé. |
| Les choix de fournisseurs | `reglages.toml`, section `[generation_son]` | Valeurs par défaut serde. **Jamais de clé.** |
| Les clés des services en ligne | **Gestionnaire d'identifiants de Windows** (Credential Manager) | Identifiants génériques `SHOT/claude`, `SHOT/elevenlabs`, `SHOT/stableaudio`. Voir §4. |

Réglages (`[generation_son]`) : `lecture`, `sons`, `voix` (`"regles"`, `"local"`, `"en_ligne"`), `lecture_service`,
`sons_service`, `voix_service` (`"claude"`, `"elevenlabs"`, `"stableaudio"`), `port_local` (8765),
`modele_claude`, `envoyer_vignettes` (vrai), `voix_locale` (`"ff_siwis"`), `voix_en_ligne` (vide), et pour le
serveur local : `modeles_local` (le dossier `SHOT-modeles` ; vide : `Documents\Work\SHOT-modeles`), `python_local` (le
`python.exe` du venv ; vide : `<modeles_local>\venv\Scripts\python.exe`), `demarrage_auto` (vrai). Le journal du
serveur lancé par SHOT : `%LOCALAPPDATA%\SHOT\serveur_son.log`.

---------------------------------------------------------------------------------------------------

## 3. Ce que SHOT envoie — et la confirmation en ligne

La lecture reçoit, pour chaque plan : une **vignette** PNG de 384 points de large (première image du plan), l'**action**,
le **dialogue**, les **notes**, les mêmes champs de ses **images de storyboard**, sa **durée** en images et la cadence, la
**transition** qui l'amène et celle qui le quitte, ses **commentaires** non résolus, et le titre / l'action des plans
voisins. Sur le fond, la même demande part vers le serveur local et vers Claude (`lecturesb.rs`).

Quand un rôle est confié à un **service en ligne** :

- le panneau dit en permanence ce qui part (« Les vignettes des plans et leurs textes seront envoyés à Claude. »,
  « Les descriptions des sons seront envoyées à ElevenLabs. », « Les répliques seront envoyées à ElevenLabs. ») ;
- avant le premier envoi d'un projet, une **fenêtre de confirmation** nomme ce qui partira et demande « Confirmer
  et continuer » ou « Annuler » ; rien ne part avant. La confirmation est retenue **pour ce projet** (dans
  `sons_proposes.json`) ; un autre projet la redemande ;
- l'option « Joindre la vignette de chaque plan » (Préférences) permet de n'envoyer que les textes.

Le lecteur par règles et le serveur local n'envoient rien hors de l'ordinateur et ne demandent rien.

---------------------------------------------------------------------------------------------------

## 4. Les clés

- Saisies dans **Fichier > Préférences > Génération sonore > « Saisir la clé… »**, dans un champ masqué, et rangées dans
  le Gestionnaire d'identifiants de Windows par `CredWriteW` (`clessecretes.rs`, déclaration à la main des fonctions
  d'`advapi32`, comme `materiel.rs` le fait pour `kernel32` : aucune caisse de plus). `CredReadW` les relit **au moment
  de l'appel**, sur le fil de travail ; elles ne sont gardées nulle part, ni dans `reglages.toml`, ni dans le projet, ni
  dans un rapport d'erreur, ni à l'écran. `Cle` et `RequeteHttps` se montrent masquées même dans `{:?}`, et leurs octets
  sont effacés quand elles disparaissent.
- « Effacer la clé » la retire du Gestionnaire. « Tester la connexion » fait une lecture qui ne coûte rien et ne dit rien
  du projet (la liste des modèles de Claude, le compte d'ElevenLabs, le solde de Stability).
- Une clé est à la taille d'un secret Windows : 2 560 octets au plus.
- Le texte tapé est effacé à la fermeture de la fenêtre de saisie.

---------------------------------------------------------------------------------------------------

## 5. Les transports : ce qui est branché, ce qui reste « à brancher »

| Transport | État |
|---|---|
| Lecture par règles | ✅ fait, sans réseau |
| **Serveur local** sur `127.0.0.1` (`httplocal.rs`, `sidecar.rs`, `serveurlocal.rs` ; serveur : `sidecar/serveur.py`) | ✅ client et serveur écrits, **essayés de bout en bout** (voix Kokoro, lecture Qwen2.5-VL) ; les bruitages attendent Stable Audio (§8.5) |
| **Services en ligne** (`fournisseursapi.rs`) | ⚠ **requêtes construites et réponses relues**, essayées contre un transport simulé ; **l'envoi (HTTPS) est « à brancher »** |

Pourquoi : le cache de caisses hors ligne n'a ni `ureq`, ni `reqwest`, ni aucune caisse TLS (le seul client TLS possible
sans en télécharger serait écrit à la main, ce qui n'est pas sérieux). `TransportABrancher` est donc le transport de
cette version : chaque appel rend « connexion en ligne à brancher : cette version de SHOT ne sait pas encore envoyer de
requête aux services en ligne (aucun module TLS dans la compilation hors ligne)… ». Brancher un transport, c'est écrire un
`impl Transport` (une méthode `envoyer(&RequeteHttps) -> ReponseHttp`) et le donner à `FabriqueReelle` ; **WinHTTP**
(`winhttp.dll`, déclaré à la main comme `advapi32`) ferait le TLS avec le magasin de certificats de Windows sans aucune
caisse — c'est la piste recommandée, à valider avec Anthony avant de faire partir une vraie requête.

Le client local est un HTTP/1.1 minimal sur `TcpStream` (`httplocal.rs`) : **adresses de boucle locale seulement**
(`127.0.0.1`, `::1` ; toute autre est refusée avant la connexion), lecture par tranches de 100 ms pour rester
**annulable** (annuler ferme la connexion, ce que le serveur voit), délai total, réponse bornée à 128 Mo, réponses à
`Content-Length`, `Transfer-Encoding: chunked` ou terminées par la fermeture.

Les adresses, en-têtes et formats des trois services en ligne sont ceux de leurs documentations **à vérifier avant le
premier vrai appel** (aucun n'a pu être essayé) :

- Claude : `POST https://api.anthropic.com/v1/messages`, en-têtes `x-api-key`, `anthropic-version: 2023-06-01`, corps
  `{model, max_tokens, system, messages:[{role:"user", content:[{type:"image", source:{type:"base64", media_type:"image/png",
  data}}, {type:"text", text}]}]}`. Le nom du modèle est un réglage (`modele_claude`, `claude-sonnet-5-5` par défaut : **à vérifier
  avec le compte de l'utilisateur**).
- ElevenLabs Sound Effects : `POST https://api.elevenlabs.io/v1/sound-generation?output_format=mp3_44100_128`, en-tête
  `xi-api-key`, corps `{text, duration_seconds (0,5 à 22), prompt_influence}` → MP3.
- ElevenLabs Text to Speech : `POST https://api.elevenlabs.io/v1/text-to-speech/{voice_id}?output_format=mp3_44100_128`,
  corps `{text, model_id: "eleven_multilingual_v2"}` → MP3. L'identifiant de voix est contrôlé (lettres, chiffres, `-`,
  `_`) pour ne pas changer l'adresse.
- API de Stable Audio : `POST https://api.stability.ai/v2beta/audio/stable-audio-2/text-to-audio`, `Authorization: Bearer`,
  formulaire `multipart/form-data` `{prompt, duration (1 à 190), output_format: "mp3"}` → MP3.

---------------------------------------------------------------------------------------------------

## 6. Le contrat du serveur local (écrit : `sidecar/serveur.py`, voir §8)

Un seul processus Python, lancé par SHOT ou par l'utilisateur, qui écoute **uniquement sur `127.0.0.1`** (jamais `0.0.0.0`),
port `8765` par défaut (réglage `port_local`). JSON UTF-8 sur HTTP/1.1, requêtes `Connection: close`. Pas
d'authentification (le serveur n'est joignable que depuis cet ordinateur) ; il refuse toute requête dont la taille dépasse
20 Mo. **Une demande à la fois** (un seul modèle en mémoire à la fois, une seule carte) : une demande arrivée pendant
qu'une autre travaille reçoit `503 occupe`. **Fermer la connexion annule la demande** : le serveur interrompt la génération
dès qu'il voit la connexion se fermer.

Délais de SHOT : 5 s pour `/v1/etat`, 600 s pour `/v1/lecture` et `/v1/son`, 300 s pour `/v1/voix` (le premier appel charge le
modèle : il peut être long).

### `GET /v1/etat`

```json
{ "version": 1,
  "modeles": {
    "lecture": { "installe": true,  "nom": "Qwen2.5-VL-7B-Instruct" },
    "son":     { "installe": false },
    "voix":    { "installe": true,  "nom": "Kokoro-82M" } } }
```

`version` est la version du **contrat** (1). `installe: false` : le modèle n'est pas là ; l'appel correspondant doit
répondre `501 non_installe`.

### `POST /v1/lecture` — lire un plan

Requête (le plan, tel que `lecturesb::plan_en_json` le fait ; la vignette est facultative) :

```json
{ "version": 1, "langue": "fr",
  "consignes": "You are a sound designer reading the storyboard of an animated film... (texte de SHOT)",
  "plan": {
    "id": 12, "titre": "Plan 3", "sequence": "1", "cadence": 24, "images": 48,
    "action": "La porte claque. Il pleut dehors.", "dialogue": "MARIE : Viens ici.", "notes": "SFX : pas dans l'escalier",
    "marques": [ { "image": 24, "action": "...", "dialogue": "...", "notes": "..." } ],
    "transition_entrante": "Fondu enchaîné, 12 images", "transition_sortante": null,
    "commentaires": ["Le flou arrive trop tôt."],
    "plan_precedent": { "titre": "Plan 2", "action": "..." }, "plan_suivant": null },
  "vignette_png_b64": "iVBORw0KGgo..." }
```

Réponse `200` :

```json
{ "indications": [
  { "image": 3, "images": 36, "genre": "bruitage", "description": "Porte qui claque",
    "prompt": "heavy wooden door slamming shut", "texte": "", "personnage": "" },
  { "image": 0, "images": 48, "genre": "voix", "description": "MARIE : « Viens ici. »",
    "prompt": "", "texte": "Viens ici.", "personnage": "MARIE" } ] }
```

`genre` : `bruitage`, `ambiance`, `musique` ou `voix`. `image` (depuis 0, dans le plan) et `images` en images de la cadence
donnée. `description` en français, `prompt` en anglais (vide pour une voix), `texte` obligatoire pour une voix. SHOT relit
la réponse avec méfiance : une sorte inconnue, une voix sans texte ou une indication vide sont écartées, les temps sont
ramenés dans le plan, les textes sont bornés (400 / 600 / 1 000 caractères), 200 indications au plus. Le serveur peut
ignorer `consignes` et utiliser les siennes.

### `POST /v1/son` — un bruitage, une ambiance ou une musique

Requête : `{ "version": 1, "genre": "ambiance", "prompt": "steady rain falling on a roof", "duree_s": 4.5 }`.
`duree_s` est la durée voulue ; le serveur la ramène à ce que le modèle sait faire (Stable Audio Open Small : environ
11 s ; 1.0 : 47 s) et **rend le son tel qu'il l'a fait** : la durée posée dans le montage est celle du fichier. Pour une
ambiance plus longue que le modèle, le serveur peut boucler en fondu enchaîné (à décider en phase 2).

Réponse `200`, l'une ou l'autre :

- `Content-Type: audio/wav` (ou `audio/mpeg`, `audio/flac`, `audio/ogg`) et les octets du fichier ;
- `{ "audio_b64": "UklGR...", "format": "wav" }` (`wav`, `mp3`, `flac` ou `ogg`).

WAV 16 bits (ou 32 bits flottants), mono ou stéréo, 44,1 ou 48 kHz : SHOT le décode (symphonia) et le rééchantillonne.

### `POST /v1/voix` — une voix témoin

Requête : `{ "version": 1, "texte": "Viens ici.", "personnage": "MARIE", "voix": "ff_siwis", "langue": "fr" }`. `voix` vide :
la voix par défaut du serveur. `personnage` peut servir à garder la même voix d'une réplique à l'autre. Réponse : comme
`/v1/son`.

### Erreurs

Toute réponse qui n'est pas `2xx` porte `{ "erreur": { "code": "...", "message": "..." } }`, `message` en français, court,
lisible par le réalisateur. SHOT s'en sert tel quel.

| HTTP | `code` | Sens pour SHOT |
|---|---|---|
| 501 | `non_installe` ou `modele_absent` | « non installé » : le modèle de ce rôle n'est pas là |
| 503 ou 429 | `occupe` ou `indisponible` | une autre demande travaille, ou plus de mémoire graphique |
| 400 ou 422 | `invalide` | la demande est inexploitable (SHOT ne réessaie pas, continue avec la suite) |
| 5xx autre | `interne` | panne du serveur : SHOT s'arrête et le dit |

`non_installe`, `occupe` et `interne` **arrêtent la tâche** en cours (rien ne changera au plan ou au son suivant) ;
`invalide` ne l'arrête pas.

Ce que `sidecar/serveur.py` fait de ce contrat : un corps de plus de 20 Mo reçoit `413 invalide` ; une version du contrat
plus récente que 1 reçoit `400 invalide` ; la mémoire graphique pleine donne `503 indisponible` ; une voix qui n'est pas
installée donne `400 invalide` avec la liste des voix ; le champ `consignes` de la lecture est ignoré et les voix d'un plan
sont tirées de ses lignes de dialogue (§8.3).

---------------------------------------------------------------------------------------------------

## 7. Choix des modèles, et pourquoi

| Rôle | Modèle | Pourquoi | Réserves |
|---|---|---|---|
| Lecture | **Qwen2.5-VL** (Instruct, 7B en priorité ; 3B si la carte est juste) | Lit des images **et** du texte, suit une consigne de sortie en JSON, comprend le français, tourne en local avec `transformers`. | Mémoire : 7B demande une carte de 16 Go en demi-précision, ou environ 8 Go quantifié (AWQ / 4 bits). Licence selon la taille. |
| Bruitages, ambiances | **Stable Audio Open** (Small ou 1.0) | Texte → audio (sons, ambiances, musique courte), en local, sans compte pour l'utilisation. **Small** (~340 M paramètres, environ 11 s) : rapide, léger ; **1.0** (~1 Md, jusqu'à 47 s stéréo 44,1 kHz) : meilleur pour les ambiances longues. | Les poids sont sur Hugging Face, **derrière l'acceptation de la licence avec un compte** : le téléchargement demande un jeton que l'utilisateur doit donner lui-même (SHOT ne le stocke pas). Licence. |
| Voix témoin | **Kokoro-82M** | 82 M de paramètres : instantané même sans carte, voix françaises (`ff_siwis`), sons propres pour un témoin d'animatique. | Il faut `espeak-ng` pour le français (phonétisation). Licence du modèle libre, de `espeak-ng` GPL (programme séparé). |
| Lecture en ligne | **Claude** | Même demande que le modèle local, meilleure compréhension des textes longs. | Voir §4 et §5. |
| Sons en ligne | **ElevenLabs Sound Effects**, **API de Stable Audio** | Bruitages de bonne qualité sans rien installer. | Quotas et prix du compte de l'utilisateur. |
| Voix en ligne | **ElevenLabs TTS** | Voix expressives en français. | Idem. |

### Licences : à vérifier (aucune n'a été relue avec le texte officiel)

SHOT produit ici des **sons témoins pour l'animatique**. Avant de garder un son dans un film diffusé, l'utilisateur
vérifie les conditions de chaque modèle et de chaque service — l'usage commercial des sons générés, la réutilisation
des données envoyées, l'attribution :

- Qwen2.5-VL-7B-Instruct-AWQ (installé) : la fiche `README.md` du dossier annonce `license: apache-2.0`, **mais le dossier
  contient aussi un fichier `LICENSE` « Qwen LICENSE AGREEMENT » (19 septembre 2024)** : les deux ne disent pas la même
  chose, **à vérifier** (la licence Qwen n'est pas Apache 2.0). Ici Qwen ne fait que lire le texte du storyboard : aucun son
  n'en sort.
- Stable Audio Open (non installé) : licence communautaire de Stability AI (conditions liées au chiffre d'affaires, jeux de
  données sous licences Creative Commons) à accepter sur la page du modèle — **à vérifier**, y compris pour un film
  commercial.
- Kokoro-82M (installé) : la fiche annonce `license: apache-2.0` (poids) ; `espeak-ng` (GPL) reste un programme à part
  (bibliothèque `espeakng-loader`), appelé par Kokoro pour le français. À vérifier.
- Le code de `sidecar/` utilise `torch` (BSD), `transformers` (Apache 2.0), `fastapi`/`uvicorn` (MIT/BSD), `kokoro`
  (Apache 2.0) : voir leurs dépôts.
- Claude, ElevenLabs, Stability AI (API) : conditions d'utilisation et politique de données du compte — **à vérifier**,
  en particulier si le projet est confidentiel (les images et les textes quittent l'ordinateur).

Une note le rappelle dans les Préférences et dans la fenêtre de confirmation.

---------------------------------------------------------------------------------------------------

## 8. Le serveur local (phase 2, branche `sidecar-son`)

Le serveur est un script lisible, versionné avec SHOT, dans **`sidecar/`** (jamais dans `SHOT-modeles`, qui ne contient
que le Python, le venv et les modèles) :

| Fichier | Rôle |
|---|---|
| `sidecar/serveur.py` | L'application FastAPI : le contrat du §6, rien de plus. `127.0.0.1` seulement ; une demande à la fois (`503 occupe`) ; fermer la connexion annule ; erreurs en JSON français ; refuse plus de 20 Mo ; quitte si SHOT disparaît (`--parent-pid`) ; rend la mémoire graphique après 10 minutes sans demande. |
| `sidecar/modeles.py` | Kokoro, Qwen (lecture), Stable Audio, et le gestionnaire de mémoire graphique (un seul gros modèle à la fois : Qwen est déchargé avant Kokoro ou Stable Audio, Kokoro avant Qwen). |
| `sidecar/awq.py` | Qwen2.5-VL **AWQ chargé sans `gptqmodel`** (voir 8.3). |
| `sidecar/config.toml` | Les noms des dossiers de modèles, la voix par défaut, la longueur de réponse. **Aucun chemin d'utilisateur** : le dossier `SHOT-modeles` est donné par SHOT (`--racine`) ou par la variable `SHOT_MODELES`. |
| `sidecar/lancer.ps1` | Lance le serveur à la main (PowerShell 5.1). |
| `sidecar/essai.py` | L'essai de fumée, à lancer à la main (`pytest` n'est pas installé dans le venv). |

### 8.1 Ce qui est installé (dans `Documents\Work\SHOT-modeles`, installé avec l'accord d'Anthony)

- `python\` : CPython 3.12.10 ; `venv\` : torch 2.14.1+cu126 (CUDA marche sur la RTX 2080 SUPER, 8 Go), transformers 5.20.1,
  accelerate, qwen-vl-utils, kokoro 0.9.4 (avec espeakng-loader), soundfile, fastapi, uvicorn, huggingface_hub.
- `modeles\Qwen2.5-VL-7B-Instruct-AWQ` (6,9 Go), `modeles\Kokoro-82M` (voix `ff_siwis`, `af_heart`, `am_michael`).
- **Pas installé** : Stable Audio Open (8.4).

### 8.2 Le lancer

- **Par SHOT** (le plus simple) : Fichier > Préférences > Génération sonore > « Serveur local » : le dossier des modèles et
  le Python (par défaut ceux de `Documents\Work\SHOT-modeles`), « Démarrer le serveur local » / « Arrêter » / « Tester ».
  Avec la case « Démarrer le serveur tout seul… » (cochée d'origine), un « Générer » local ou une lecture par Qwen lance le
  serveur si personne n'écoute, **en arrière-plan, sans fenêtre**, attend qu'il réponde et reprend. SHOT l'arrête en se
  fermant (le processus est dans un objet de travail Windows qui le tue même si SHOT plante, avec le Python de base que le
  lanceur du venv démarre). Un serveur lancé à la main n'est jamais arrêté par SHOT. Rien ne bloque l'interface : le
  lancement, l'attente et l'arrêt se font sur des fils de travail ; le chargement d'un modèle se fait dans le serveur.
  Le journal du serveur : `%LOCALAPPDATA%\SHOT\serveur_son.log`.
- **À la main** : `.\sidecar\lancer.ps1 [-Racine D:\SHOT-modeles] [-Port 8765]`, ou
  `<venv>\Scripts\python.exe -I sidecar\serveur.py --racine <SHOT-modeles> --port 8765`.
- **Essai de fumée** : `<venv>\Scripts\python.exe -I sidecar\essai.py --racine <SHOT-modeles> [--sans-lecture]`.

### 8.3 Qwen2.5-VL en AWQ sans `gptqmodel`

`transformers` 5.20.1 refuse un modèle AWQ tant que **`gptqmodel`** n'est pas installé (« Loading an AWQ quantized model
requires gptqmodel »), et ce paquet compile des noyaux CUDA : non installé, par la règle « aucun téléchargement ». Plutôt
que de laisser la lecture par Qwen hors d'usage, `sidecar/awq.py` lit les poids AWQ (4 bits, groupes de 128) directement dans
les fichiers safetensors et les déquantifie couche par couche dans PyTorch. Les poids restent en 4 bits dans la carte ;
les plongements (1,1 Go, une table de consultation) restent dans la mémoire de l'ordinateur ; la tour visuelle (1,4 Go) vient
sur la carte le temps de lire la vignette, puis repart. Le prix : pas de noyau CUDA dédié, donc **environ 4 jetons par seconde**.
Pour que cela reste supportable, le serveur demande à Qwen une réponse courte (une ligne par bruitage, ambiance ou musique)
et construit **lui-même les voix à partir des lignes de dialogue** (comme `lecturerules.rs` : une voix par ligne, l'une après
l'autre) : jamais de réplique déformée, et la moitié des jetons en moins. Le serveur ignore donc le champ `consignes` de
SHOT (le contrat le permet) ; il bride aussi un bruitage à 3 s et une ambiance à 30 s.

### 8.4 Mesures (RTX 2080 SUPER 8 Go, dont ~1 à 2 Go déjà pris par Windows et les autres programmes)

| | Mesure |
|---|---|
| Kokoro, premier appel (importer torch, spaCy, kokoro ; charger le modèle) | 17 à 20 s, une fois par démarrage du serveur |
| Kokoro, ensuite | 0,1 à 0,2 s pour 2 à 7 s de parole (30 à 50 fois plus vite que le temps réel) ; 24 kHz, WAV 16 bits mono |
| Kokoro, mémoire graphique | environ 0,95 Go (contexte CUDA compris) |
| Kokoro rechargé après Qwen | 1 s |
| Qwen2.5-VL AWQ, chargement (fichiers déjà dans le cache de Windows) | 3 à 4 s ; compter plus pour une première lecture du disque (6,9 Go) |
| Qwen, mémoire graphique | 4,6 Go réservés au repos, 5,7 Go au pic (lecture d'une vignette) : tient dans 8 Go **si rien d'autre ne prend plus de ~2 Go** ; SHOT lui-même et Windows comptent |
| Qwen, un plan (texte seul ou avec vignette) | 12 à 19 s : 4 jetons par seconde, 50 à 60 jetons de réponse |
| Lecture d'un film de 30 plans | de l'ordre de 7 à 8 minutes ; les règles (par défaut) le font instantanément |
| Fermer la connexion pendant que Qwen écrit | le serveur est libre 2 s plus tard |
| De bout en bout dans SHOT (essai `sidecar_reel_*`) | voix : « Générer » → fichier WAV → « prête » → « Accepter » → posée sur la piste « Sons proposés » : 21 s la première fois (le serveur démarre et Kokoro se charge), 0,13 s ensuite. Lecture Qwen d'un plan avec sa vignette : 33 s en tout (serveur + modèle + plan). |

### 8.5 Ce qui manque

- **Stable Audio Open** (bruitages, ambiances, musique) : `GET /v1/etat` répond `son: installé false`, `/v1/son` répond
  `501 modele_absent` avec la raison, et « Générer » dit « non installé » dans SHOT. Le code de `/v1/son` est écrit
  (`stable-audio-tools`, 8 étapes « pingpong », durée voulue respectée jusqu'à ~11,9 s) et sa plomberie est essayée avec un
  **faux** module, mais **le vrai modèle n'a jamais tourné** ici. Il faut :
  1. **accepter la licence** de `stabilityai/stable-audio-open-small` sur huggingface.co/stabilityai/stable-audio-open-small
     (modèle à accès restreint), puis **se connecter soi-même** avec `huggingface-cli login` ou `hf auth login` en tapant
     son propre jeton — SHOT ne le demande, ne le lit et ne le range nulle part ; le téléchargement se fait ensuite dans
     `SHOT-modeles\modeles\stable-audio-open-small\` (`model.safetensors` ≈ 1,2 à 1,5 Go + `model_config.json`) ;
  2. l'encodeur de texte **`google-t5/t5-base`** (public, ≈ 0,9 Go) dans `SHOT-modeles\modeles\t5-base\` : la configuration de
     Stable Audio Open Small le nomme et la bibliothèque irait le chercher en ligne ;
  3. la bibliothèque Python **`stable-audio-tools`** (`pip install stable-audio-tools` ; la roue est petite mais elle tire
     beaucoup de dépendances absentes du venv — `einops`, `k-diffusion`, `alias-free-torch`, `vector-quantize-pytorch`,
     `x-transformers`, `laion-clap`, `gradio`, `wandb`… — de l'ordre de 0,3 à 0,6 Go **à vérifier au téléchargement**) ; elle
     a été écrite pour `transformers` 4 : un conflit avec le 5.20.1 du venv est possible, auquel cas il faudra un second venv ou
     passer par `diffusers` (`pip install diffusers`, petit), que le serveur sait aussi lire pour Stable Audio Open 1.0.
- **Qwen plus rapide** : un noyau AWQ (`gptqmodel`, à compiler pour Windows) ferait passer la lecture de ~4 à plusieurs dizaines
  de jetons par seconde. Rien n'est installé pour cela.
- Les services en ligne (§5) et le cache des sons générés (jamais nettoyé) restent à faire.
- Une carte plus grande, ou rien d'autre sur la carte pendant que Qwen lit, laisse plus de marge : si la mémoire manque, le
  serveur répond `503 indisponible` avec le message « Plus assez de mémoire graphique… » et SHOT s'arrête en le disant.

---------------------------------------------------------------------------------------------------

## 9. Le code

| Fichier | Rôle |
|---|---|
| `indications.rs` | La proposition (`Indication`, `Genre`, `Statut`), `sons_proposes.json`, la fusion d'une relecture, la piste « Sons proposés » |
| `lecturesb.rs` | L'entrée de la lecture (vignette, textes, durée, transitions, commentaires), les consignes, la réponse d'un modèle |
| `lecturerules.rs` | Le lecteur sans modèle |
| `fournisseurs.rs` | Les traits `LecteurStoryboard`, `GenerateurSon`, `GenerateurVoix`, `Controle` (annulation, avancement), `ErreurGen` |
| `httplocal.rs`, `sidecar.rs` | Le client HTTP de boucle locale et le contrat du serveur local |
| `serveurlocal.rs` | Le serveur lancé par SHOT : chemins (`Documents\Work\SHOT-modeles` par défaut), processus sans console dans un objet de travail Windows, attente, arrêt, démarrage automatique (`Demarrage`) |
| `sidecar/` | Le serveur Python (voir §8) |
| `fournisseursapi.rs` | Claude, ElevenLabs, Stable Audio : requêtes, réponses, erreurs, `Transport` (à brancher) |
| `clessecretes.rs` | Le Gestionnaire d'identifiants de Windows (`CredWriteW` / `CredReadW` / `CredDeleteW`) |
| `genreglages.rs` | Les réglages (`[generation_son]`) |
| `generation.rs` | Les fils de travail, les tâches, la fabrique des fournisseurs |
| `suggestionsui.rs`, `genprefsui.rs` | Le panneau, ses fenêtres ; la section des Préférences |
| `base64.rs` | Base64, pour les vignettes et les sons dans le JSON |

Essais Rust : `indications_tests`, `fournisseurs_tests` (avec les fournisseurs simulés), `suggestions_tests`,
`base64_tests`, `serveurlocal_tests` (14 : le contrat contre un **faux serveur** qui rend les réponses relevées sur le vrai
`serveur.py`, le démarrage automatique avec un faux lanceur, et un vrai processus `cmd` pour l'arrêt de tout l'arbre). Aucun
n'ouvre le réseau (sauf `127.0.0.1` contre un serveur de l'essai), ne lance Python ni un modèle. Deux essais **ignorés**
(`sidecar_reel_*` dans `suggestions_tests.rs`) lancent le vrai serveur et se lancent à la main :
`cargo test --release --offline --target-dir target\msvc --lib sidecar_reel -- --ignored --nocapture --test-threads=1`.
Côté Python : `sidecar/essai.py` (§8.2).
Le seul essai qui touche au système est celui du Gestionnaire d'identifiants de Windows, sous un nom d'essai
(`SHOT-essai-<pid>/…`) qu'il efface.
