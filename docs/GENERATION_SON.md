# Génération sonore à partir du storyboard

Branche `generation-son` (coupée de `transitions-son`, `5598977`). **Phase 1 : tout ce qui se passe du côté
de SHOT.** La phase 2 — installer le serveur Python et les modèles — attend l'accord d'Anthony pour chaque
téléchargement (liste en fin de document). Rien n'a été téléchargé, rien n'a été envoyé sur le réseau, aucun
modèle n'a été lancé pendant cette phase : les essais utilisent des fournisseurs simulés.

Ce document sert à trois choses : dire ce que SHOT fait maintenant, **fixer le contrat** que le serveur de
la phase 2 devra respecter, et lister ce qu'il faudra télécharger.

---------------------------------------------------------------------------------------------------

## 1. Ce que fait la fonction

SHOT lit le storyboard et **propose des sons témoins**, de qualité animatique : des bruitages, des
ambiances, une musique d'intention, des voix pour les dialogues. Le réalisateur les écoute et les accepte, les
régénère ou les supprime. Rien n'entre dans le montage sans son accord.

- Panneau **Suggestions sonores** (barre des panneaux, Son > Suggestions sonores…, commande
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
tant que le serveur local n'est pas installé et qu'aucun service n'est branché, « Générer » répond « non installé » ou
« connexion en ligne à brancher ».

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
`modele_claude`, `envoyer_vignettes` (vrai), `voix_locale` (`"ff_siwis"`), `voix_en_ligne` (vide).

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
| **Serveur local** sur `127.0.0.1` (`httplocal.rs`, `sidecar.rs`) | ✅ client écrit et essayé contre un petit serveur de l'essai ; **le serveur lui-même n'existe pas encore** (phase 2) : « Tester » répond « non installé » |
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

## 6. Le contrat du serveur local (à écrire en phase 2)

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

- Qwen2.5-VL : licence variable selon la taille (Apache 2.0 pour certaines, licence Qwen pour d'autres) — **à vérifier**
  pour la taille choisie.
- Stable Audio Open : licence communautaire de Stability AI (conditions liées au chiffre d'affaires, jeux de données
  sous licences Creative Commons) — **à vérifier**, y compris pour un film commercial.
- Kokoro-82M : licence Apache 2.0 annoncée — **à vérifier** ; `espeak-ng` (GPL) reste un programme à part.
- Claude, ElevenLabs, Stability AI (API) : conditions d'utilisation et politique de données du compte — **à vérifier**,
  en particulier si le projet est confidentiel (les images et les textes quittent l'ordinateur).

Une note le rappelle dans les Préférences et dans la fenêtre de confirmation.

---------------------------------------------------------------------------------------------------

## 8. Phase 2 : ce qu'il faudrait télécharger (à confirmer avec Anthony, élément par élément)

**Rien de ceci n'est fait.** Les tailles sont des ordres de grandeur à vérifier au moment du téléchargement ; le total
dépend des choix.

| # | Élément | Taille approximative | Remarque |
|---|---|---|---|
| 1 | Python 3.11 ou 3.12 (si absent) + un environnement virtuel dédié (`%LOCALAPPDATA%\SHOT\serveur_son\`) | 0,1 Go | Hors du dossier du projet. |
| 2 | PyTorch (version CUDA) | 2,5 à 3 Go | Dépend de la carte graphique d'Anthony (CUDA ou processeur) : à lire avec le diagnostic de SHOT. |
| 3 | Bibliothèques Python : `transformers`, `accelerate`, `qwen-vl-utils`, `stable-audio-tools` (ou `diffusers`), `kokoro`, `soundfile`, `fastapi` + `uvicorn` (ou la bibliothèque standard seule) | 0,5 à 1 Go | La liste exacte avec les versions fixées sera donnée avant. |
| 4 | **Qwen2.5-VL-7B-Instruct** (poids) — ou 3B | ~16 Go (7B) ; ~5 à 6 Go quantifié ; ~7 Go (3B) | Le choix dépend de la mémoire de la carte. |
| 5 | **Stable Audio Open Small** (poids) | ~1,5 Go | Recommandé pour commencer (rapide). |
| 6 | **Stable Audio Open 1.0** (poids) | ~5 Go | Facultatif : ambiances plus longues. Compte Hugging Face et acceptation de la licence. |
| 7 | **Kokoro-82M** (poids et voix) + `espeak-ng` | ~0,4 Go | |
| | **Total** | **environ 12 à 30 Go** | Selon les choix 4 à 6. |

Avant tout téléchargement : confirmer la carte graphique (mémoire), les modèles retenus (4 à 6), l'emplacement (disque et
dossier), et qui donne le jeton Hugging Face (SHOT ne le stocke pas). Le serveur sera un script lisible, versionné avec
SHOT, qui implémente exactement le §6 ; `GET /v1/etat` dira ce qui est installé.

---------------------------------------------------------------------------------------------------

## 9. Le code

| Fichier | Rôle |
|---|---|
| `indications.rs` | La proposition (`Indication`, `Genre`, `Statut`), `sons_proposes.json`, la fusion d'une relecture, la piste « Sons proposés » |
| `lecturesb.rs` | L'entrée de la lecture (vignette, textes, durée, transitions, commentaires), les consignes, la réponse d'un modèle |
| `lecturerules.rs` | Le lecteur sans modèle |
| `fournisseurs.rs` | Les traits `LecteurStoryboard`, `GenerateurSon`, `GenerateurVoix`, `Controle` (annulation, avancement), `ErreurGen` |
| `httplocal.rs`, `sidecar.rs` | Le client HTTP de boucle locale et le contrat du serveur local |
| `fournisseursapi.rs` | Claude, ElevenLabs, Stable Audio : requêtes, réponses, erreurs, `Transport` (à brancher) |
| `clessecretes.rs` | Le Gestionnaire d'identifiants de Windows (`CredWriteW` / `CredReadW` / `CredDeleteW`) |
| `genreglages.rs` | Les réglages (`[generation_son]`) |
| `generation.rs` | Les fils de travail, les tâches, la fabrique des fournisseurs |
| `suggestionsui.rs`, `genprefsui.rs` | Le panneau, ses fenêtres ; la section des Préférences |
| `base64.rs` | Base64, pour les vignettes et les sons dans le JSON |

Essais (83) : `indications_tests`, `fournisseurs_tests` (avec les fournisseurs simulés), `suggestions_tests`,
`base64_tests`. Aucun n'ouvre le réseau (sauf `127.0.0.1` contre un serveur de l'essai) ni ne lance de modèle.
Le seul essai qui touche au système est celui du Gestionnaire d'identifiants de Windows, sous un nom d'essai
(`SHOT-essai-<pid>/…`) qu'il efface.
