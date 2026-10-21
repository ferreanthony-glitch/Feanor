# Travailler à plusieurs sur un même film (2026-10-20, branche `cooperatif`)

Demande d'Anthony : « Rendre le logiciel coopératif : possibilité de travailler à plusieurs sur un seul fichier. »
Ses réponses : une **option A**, un **dossier partagé** où chacun **prend** les plans qu'il travaille (un seul à la fois par
plan), construite pour pouvoir passer plus tard à un **serveur SHOT** ; deux personnes pour l'instant (prévoir quelques-unes) ;
au bureau **et** à distance ; chacun a sa séquence ou ses plans (jamais deux personnes qui dessinent dans le même plan) ;
stockage aujourd'hui : Google Drive, Dropbox, un serveur du studio (NAS, SMB) ; tout le monde peut prendre des plans (pas de
rôles pour l'instant).

Ce document est la conception, écrite **avant** le code. La fin dit ce qui est fait, ce qui ne l'est pas, et comment l'essayer.

## 1. Ce que le dossier partagé n'est pas

Un dossier Dropbox / Drive n'est **pas un disque** : c'est un client de synchronisation qui recopie les fichiers entre les
postes, plus tard et pas dans l'ordre.

| Ce qu'on croit | Ce qui se passe vraiment | Ce que le protocole en fait |
|---|---|---|
| Un fichier écrit est vu tout de suite par l'autre | Il arrive après quelques secondes à quelques minutes, parfois **le fichier d'un plan avant ses dessins**, ou la structure avant le plan | rien n'est jamais supposé présent : un plan dont les dessins manquent est « en attente », jamais « vide » ; on ne le prend pas, on ne l'enregistre pas |
| Un verrou de fichier (`LockFile`) protège | Aucun client de synchronisation ne propage les verrous du système | les verrous sont **des fichiers ordinaires** (une fiche par équipier), jamais des verrous du système |
| Un renommage est atomique | Seulement sur le poste qui le fait ; l'autre voit d'abord le `.tmp`, puis le fichier | les fichiers `*.tmp` / `*.part` sont ignorés par la lecture ; un fichier à moitié arrivé est « illisible pour l'instant » et se relit au tour suivant, sans message |
| Deux écritures sur un même fichier : la dernière gagne | Dropbox garde les deux : « `plan 12 (Copie en conflit de Léa 2026-10-20).json` » ; Drive peut créer « `plan (1)` » ; OneDrive ajoute le nom du poste | **chaque fichier partagé n'a qu'un seul auteur à la fois** (un plan = son porteur, une fiche d'équipier = son équipier, les commentaires = un fichier par auteur). Quand même une copie en conflit apparaît, SHOT la **repère** et propose de l'importer comme version : rien ne s'écrase en silence |
| L'horloge est la même partout | Deux postes diffèrent de quelques secondes, parfois de minutes | l'ordre des prises est le même pour tous (calculé sur les mêmes fichiers) ; la péremption d'un bail tient compte du décalage et des changements vus |
| Une prise est instantanée | La prise de Léa n'arrive chez Anthony que dans quelques secondes : ils peuvent prendre le même plan | la course est tranchée **de la même façon partout**, et le perdant ne perd rien (§6) |
| Le NAS / SMB est comme un disque | Plus proche : quasi immédiat, mais l'écriture peut échouer (réseau coupé) | mêmes règles ; un échec d'écriture est un message, le travail reste en mémoire et se réenregistre |

Règle générale : **aucune décision ne dépend de l'ordre d'arrivée des fichiers**, seulement de leur contenu ; **rien ne s'efface**
dans le dossier partagé par l'effet d'un autre poste (seules les fiches d'équipier périmées depuis plus de 30 jours sont
nettoyées, et seulement par SHOT au démarrage) ; tout désaccord finit en **brouillon** lisible, pas en perte.

## 2. Les couches

```
 interface (cooperationui.rs)          badges, fenêtre « Équipe », prendre / rendre, messages
     │
 session (cooperation.rs)              le modèle : qui tient quoi, rafraîchissement, conflits, fusion des commentaires
     │                                 un fil de fond ; l'interface ne fait jamais d'entrée-sortie
 dépôt (depot.rs, trait `Depot`)       TOUT ce qui touche le stockage : lire / écrire un plan, la structure, les fiches,
     │                                 surveiller les changements, repérer les copies en conflit
     ├── DossierPartage                 dossier local ou synchronisé (Drive, Dropbox, NAS) — fait ici
     └── ServeurShot                    plus tard : mêmes appels par le réseau (§14)
 format partagé (projet_partage.rs)    la disposition des fichiers, la conversion, l'ouverture, l'enregistrement
     │
 projet classique (project.rs)         inchangé pour un projet non partagé
```

`Depot` est **la** couture : la session ne connaît ni les chemins ni `std::fs`. Un `ServeurShot` n'aurait qu'à répondre aux
mêmes méthodes (lire / écrire / surveiller depuis un curseur) ; les dessins (`dessins/*.png`) passeraient par
`Depot::dessins_prets` / un dossier local de travail que le serveur remplirait.

## 3. La disposition d'un projet partagé

Un projet classique garde **un manifeste** pour tous les plans : deux personnes qui enregistrent s'écraseraient. Un projet
**partagé** est éclaté pour que chaque fichier n'ait qu'un auteur à la fois :

```
MonFilm/
  MonFilm.pelure           LA STRUCTURE (format 34) : séquences, ordre des plans, noms et numéros, pistes son, guides,
                           groupes de couleur, propriétés, cadence, feuille… + un bloc « partage » (voir plus bas).
                           Écrite par qui tient le jeton du montage.
  plans/<n° de plan>.json  UN PLAN : ses calques, sa caméra, ses marques, ses textes, sa durée, ses versions rangées,
                           son contenu rogné, et l'empreinte du fichier de chacun de ses dessins.
                           Écrit par qui tient le plan.
  dessins/<n°>.png|json    un fichier par dessin (comme avant) ; écrits par qui tient le plan qui les cite
  sons/ modeles/ textures/ references/ videos/         comme avant ; ajoutés par qui tient le jeton
  transitions.json effets_son.json personnages.json sons_proposes.json references.json envois.json versions/
                           comme avant, au nom d'avant ; écrits par qui tient le jeton. (Un ancien `scenes3d.json`, les décors 3D,
                           et un ancien `crayonnes.json`, la planche de crayonnés, ne sont plus lus ni écrits depuis le 2026-10-21,
                           jamais effacés.)
  commentaires/<équipier>.json     les commentaires de CET équipier (un fichier chacun, fusionnés à la lecture)
  partage/
    equipe/<équipier>.<séance>.json   la FICHE d'un équipier : qui il est, ce qu'il tient (§5), où il est (§9) ; une
                                      fiche par fenêtre SHOT ouverte, écrite par elle seule
    membres/<équipier>.json           qui a rejoint le film, sa couleur, son préfixe de numéros (§4)
    brouillons/<…>.json               travail mis de côté plutôt que perdu (§6, §8)
    importes/                         copies en conflit déjà importées (déplacées, jamais effacées)
```

Le bloc `partage` du fichier de structure : `{ "projet": "<hasard>", "revision": 17, "enregistre_par": { "id", "nom", "le" },
"plans": [{ "scene": 12, "plan": 13 }…] }`. L'ordre du film est celui des séquences et des scènes du manifeste (comme avant) ;
`plans` dit quel fichier de `plans/` est le plan de chaque scène (un plan = une scène à un panneau depuis le format 12).

**Les dessins et leurs empreintes.** Un plan ne cite pas seulement ses dessins : il porte, pour chacun, l'**empreinte de son
fichier** telle que son porteur l'a écrite (`contenu.dessins`, 64 bits sur les octets du `.png` ou `.json` et de ses annexes
`_fond`, `_coupe`, `_hors`). C'est ce qui rend sûres deux choses qu'un dossier synchronisé ne garantit pas :

- **un plan n'est pris que complet** : un lecteur compare les fichiers qu'il a sous les yeux à ces empreintes ; si l'un manque
  (la synchronisation n'a pas livré le dessin) ou diffère (elle a livré le plan avant le nouveau dessin, qui garde le **même nom**
  que l'ancien), le plan reste « en attente » et le plan déjà affiché n'est pas remplacé ;
- **un plan est « modifié » même si seul un pixel a changé** : le fichier du plan ne dit pas les pixels, mais l'empreinte du
  dessin change, donc celle du contenu du plan, donc sa révision (sans cela, retoucher un dessin n'aurait réveillé personne).

**Format.** `project::VERSION` reste **33** pour tout projet classique : un projet non partagé s'écrit, s'ouvre et se relit
**exactement comme avant**, sans migration. Le fichier de structure d'un projet partagé porte `"version": 34` : une version
de SHOT qui ne connaît pas le partage le **refuse** (« créé par une version plus récente ») au lieu de montrer un film sans
plans. Les fichiers de plan, de fiche, de membre et de commentaires ont chacun leur `format` et leur `version` (1), refusés
quand ils sont plus récents, jamais effacés quand ils sont illisibles (`write_sidecar`).

**Ce qui est au plan, ce qui est au montage.** Au plan (il faut tenir le plan) : tout ce qui se dessine ou se règle dans le
plan — calques, dessins, expositions, effets, caméra, marques d'image, textes d'action / dialogue / notes, signets,
versions rangées, contenu rogné. Au montage (il faut tenir le jeton) : ordre, nombre, noms et numéros des
plans et des séquences, **durée** d'un plan, rognage du montage, pistes et extraits de son, transitions, guides, groupes,
planche (notes), fond, cadence, propriétés, envois, références, verrous du storyboard et de la numérotation.
**Les commentaires, eux, n'ont pas besoin de rien tenir** : on peut commenter n'importe quel plan.

*Décision — durée d'un plan.* Changer la durée d'un plan (la fiche du plan, la poignée de la fin, rogner) décale tous les plans
d'après et laisse le son où il est : c'est une opération de **montage**, donc le jeton. En revanche **animer au-delà de la fin**
(dessiner une image de plus) rallonge le plan tout seul, comme avant : c'est du contenu du plan, son porteur le fait sans
jeton. Les autres voient la nouvelle durée à leur prochain rafraîchissement et un message le dit (« le plan 12 passe de 24
à 36 images ») ; le son ne bouge pas, comme dans un projet seul.

## 4. Des numéros qui ne se rencontrent jamais

Dans un projet classique, un seul compteur donne les numéros de dessins, de plans, de séquences, de marques, de commentaires,
de pistes, de sons, de transitions… Deux personnes qui créent chacune un dessin 41 se marcheraient dessus (`dessins/41.png`).

Dans un projet partagé, chaque équipier reçoit à son arrivée un **préfixe** (20 bits, tiré au hasard, vérifié contre les
autres membres) et tous ses numéros neufs sont `préfixe · 2³² + compteur` : au plus 2⁵², donc sans perte dans un JSON.
Les numéros d'avant (préfixe 0, d'un projet converti) restent tels quels. Un numéro dit donc aussi **qui l'a créé**
(`numero >> 32`). Le préfixe est retenu dans `partage/membres/<équipier>.json` ; deux arrivées simultanées avec le même
tirage sont si improbables (1 sur un million) que le seul garde-fou est la vérification au moment de rejoindre, et le fait que
le compteur de chacun repart au-dessus du plus grand numéro de son préfixe qu'il a lu.

## 5. Prendre un plan : les fiches et la course

### La fiche d'équipier

`partage/equipe/<équipier>.<séance>.json` (une par fenêtre SHOT ; deux fenêtres du même équipier sont deux « séances » : elles se
disputent les plans comme deux personnes, ce qui permet d'essayer à deux fenêtres sur un seul poste) :

```json
{ "format": "shot-equipier", "version": 1,
  "equipier": { "id", "nom", "couleur", "machine", "prefixe" }, "seance": "…",
  "renouvele_le": 1760968800000, "sequence": 41,
  "prises": [ { "cible": "plan:5", "pris_le": …, "revision": 7, "remplace": [ { "equipier", "seance", "pris_le" } ] },
              { "cible": "montage", "pris_le": …, "revision": 17 } ],
  "rendues": [ { "cible": "plan:6", "le": …, "revision": 9 } ],
  "presence": { "plan": 5, "mode": "Animation", "vu_le": … } }
```

Une seule écriture par fiche à la fois, par son seul auteur : **aucun conflit d'écriture possible**, même sur Dropbox.
Elle est réécrite quand quelque chose change (prise, rendu, enregistrement, autre plan regardé : au plus une fois toutes les
cinq secondes) et **toutes les 30 s** tant que SHOT est ouvert (le bail). Un plan se prend, un plan se rend : « rendu » garde la
dernière `revision` écrite, pour que le suivant sache quel fichier attendre (§5.4).

### 5.1 Qui tient un plan

Le **titulaire** d'une cible (un plan, ou le jeton du montage) se calcule sur **toutes les fiches lues**, de la même façon sur
tous les postes :

1. on garde les prises non rendues et **vivantes** (bail non expiré, §5.2) ;
2. on écarte celles que **remplace** une prise vivante ou rendue (la reprise d'une prise périmée, §5.3) ;
3. le titulaire est la plus ancienne (`pris_le`), à égalité le plus petit identifiant d'équipier puis de séance.

Un état pour moi, par plan : **libre**, **à moi** (provisoire pendant les quelques secondes où une prise plus ancienne chez
l'autre pourrait encore arriver), **à X**, **à X, sans nouvelles** (bail expiré : à reprendre), **perdu** (j'avais pris, l'autre était
là avant).

### 5.2 Le bail

Un bail se renouvelle toutes les 30 s ; il est **expiré** après 10 minutes sans renouvellement (réglable dans les
préférences). La péremption ne se fie pas à la seule horloge de l'autre poste : une prise est vivante si son `renouvele_le` est
récent **ou** si nous avons vu sa fiche changer récemment (nos propres minutes, pas les siennes). Une fiche d'un jour vue
pour la première fois est donc périmée tout de suite, et une horloge en retard d'une heure ne fait pas « mourir » quelqu'un qui
travaille.

### 5.3 Reprendre

Une prise expirée se **reprend** après une confirmation (« Léa n'a pas donné signe de vie depuis 14 min. Reprendre le plan 12 ?
Ce qu'elle n'a pas enregistré sera mis de côté si elle revient. »). La fiche de qui reprend porte `remplace` : la prise écartée
**reste écartée pour toujours**, même si Léa se réveille et renouvelle son bail (son horloge a recommencé) : elle voit qu'elle a
perdu le plan (§6, §8). Reprendre une prise **vivante** n'existe pas.

### 5.4 Prendre dans un état à jour

Prendre un plan, c'est d'abord s'assurer qu'on a **le dernier fichier** : la dernière fiche qui le tenait dit quelle
`revision` elle a écrite ; tant que le plan lu ici est plus ancien (la synchronisation n'a pas fini) ou qu'un de ses dessins
manque, le plan est « en synchronisation » : la prise est enregistrée mais le dessin reste fermé (« je vérifie que le plan est
à jour… »). Après 60 s, SHOT propose de forcer (« reprendre sans attendre » : le plan lu ici sert de base, et ce que l'autre
avait écrit en plus arrivera comme conflit).

### 5.5 Une séquence, plusieurs plans, le jeton

« Prendre la séquence » prend tous ses plans libres d'un coup (une écriture de fiche) ; s'il y en a un tenu par un autre, il est
laissé et dit. Le **jeton du montage** est une cible comme une autre (`montage`), avec le même bail, la même course, la même
reprise. Sans lui on dessine dans ses plans, on commente, on lit, on exporte ; avec lui on monte.
Un plan qu'on **crée** (nouveau plan, copie, coupe) est à soi dès sa naissance ; un plan qu'on veut supprimer, fusionner ou couper
doit être à soi ou libre (pris au passage) : on ne détruit pas le plan que Léa dessine.

*Prise automatique* (préférence, éteinte par défaut) : la première modification d'un plan libre le prend ; sinon la modification
est refusée avec le message « Ce plan est libre : clic droit > Prendre ce plan ».

## 6. La course, la perte, les brouillons

Léa et Anthony prennent le plan 12 à quelques secondes d'intervalle, avant que la fiche de l'un n'arrive chez l'autre. Chacun
se croit titulaire (c'est le cas « provisoire »). Quand les deux fiches sont lues, le §5.1 désigne le même gagnant partout (la
plus ancienne prise). Pour le perdant :

1. le plan se ferme tout de suite (lecture seule) et un message le dit (« Léa avait pris le plan 12 avant toi. Ton travail est
   gardé. ») ;
2. **ce qu'il avait déjà fait dans le plan est écrit dans `partage/brouillons/plan-12.<équipier>.<heure>.perdu.json`** (le fichier de
   plan complet). Les dessins qu'il a retouchés ou créés depuis qu'il a reçu le plan y prennent des **numéros neufs, à lui** et
   s'écrivent à côté des autres dans `dessins/` : rien de ce que le porteur a dans `dessins/` n'est écrasé ;
3. le plan reprend le contenu du gagnant dès qu'il arrive (le plan lu ici n'est jamais jeté avant qu'un brouillon existe) ;
4. le porteur du plan voit « 1 brouillon de Léa pour ce plan » et peut l'**importer comme version** : « V3 — conflit de Léa »
   (réutilise les versions de plans, `planvers.rs` : la version rangée porte la date et le nom de son auteur en tête de ses notes).

Le même chemin sert quand un plan est **repris** pendant qu'on travaillait hors connexion (§8), quand on a modifié un plan sans
le tenir (garde-fou du §10), et quand le jeton est perdu.

## 7. Rafraîchir sans déranger

Un fil de fond (`cooperation.rs`) fait, toutes les 3 s :

1. `Depot::surveiller(curseur)` : **un seul balayage peu coûteux** (date et taille des fichiers de `plans/`, des fiches, de
   `commentaires/`, de la structure ; sur Windows la date vient avec la liste du dossier, sans un appel de plus par fichier) ;
2. s'il y a du nouveau : les fiches sont relues (prises, présence, membres), les commentaires fusionnés, les plans changés
   **lus et décodés sur ce fil** (PNG, traits) en un `PlanCharge` complet ;
3. l'interface reçoit les résultats par un canal et ne fait que les **poser** : remplacer le panneau du plan, ses dessins,
   ses versions ; la pose d'un plan prend de l'ordre de la microseconde par calque (mesures plus bas), elle ne décode rien ;
4. le battement : toutes les 30 s (ou quand quelque chose change) la fiche de l'équipier est réécrite.

Ce qu'un rafraîchissement **ne touche jamais** : un plan que je tiens (même si un fichier plus récent arrive, ce qui n'arrive
que par une reprise, §6), le travail non enregistré, l'historique d'annulation (§10). Les plans des autres sont remplacés ; leur
numéro de version de dessin augmente pour que les vignettes et les caches se refassent.

Une **notification** (« Léa a enregistré le plan 12 », « Léa a pris la séquence 2 », « le montage a changé ») est ajoutée à
la liste de la fenêtre Équipe et montrée dans la barre d'état. Rien ne s'ouvre, rien ne bouge sous la main.

**Copies en conflit.** Le balayage repère aussi les noms que fabriquent les clients de synchronisation dans `plans/`, `commentaires/`
et à côté de la structure (« … (Copie en conflit de … 2026-10-20) », « … (conflicted copy) », « … (1) », « …-NOMDUPOSTE ») : le
fichier est lu comme un plan, et la fenêtre Équipe propose **« Importer comme version »** (puis il est déplacé dans
`partage/importes/`, jamais effacé) ou « Ignorer ».

## 8. Hors connexion

On peut continuer à travailler dans les plans qu'on tient sans réseau : tout est en mémoire, l'enregistrement dans le dossier
local de synchronisation marche (Dropbox/Drive rattrapent plus tard), un dossier réseau coupé donne un message d'échec et le
projet reste « modifié ». À la reconnexion SHOT relit les fiches : si **on tient toujours** le plan, rien ne change ; si
quelqu'un l'a **repris** (bail expiré), le plan se ferme et le travail non encore écrit devient un **brouillon** (§6), importable
comme version : rien n'est perdu. La copie de secours (`recovery.rs`) reste locale et inchangée : elle enregistre un projet
classique complet dans le dossier de secours de SHOT.

## 9. Présence

La fiche porte `presence` : le plan regardé, le mode (Storyboard / Animation), `vu_le`. Un équipier est **en ligne** si une de ses
fiches a changé il y a moins de 90 s. L'interface montre, sur chaque bloc du montage et sur chaque vignette de la planche, une
**pastille de la couleur du porteur** avec ses initiales (plan pris), une pastille creuse pour ceux qui **regardent** ce plan,
et un cadenas gris pour un plan en attente. La fenêtre « Équipe » (menu Fichier, ou l'icône à droite de la barre d'état) liste
qui est en ligne, ce qu'il tient, les notifications et les copies en conflit.

Pas de curseur partagé, pas de flux d'images : la présence est de l'information, pas du temps réel.

## 10. Enregistrer, annuler, ne rien perdre

**Enregistrer** un projet partagé (Ctrl+S, même chemin d'écriture que le reste : cliché sur l'interface, écriture sur un fil,
fichiers atomiques `.tmp` puis renommage, jamais un fichier illisible effacé) n'écrit que :

1. les **dessins** des plans que je tiens et qui ont changé ;
2. les **plans** que je tiens dont le contenu a changé (comparaison d'empreinte : un plan inchangé n'est pas réécrit, sa date
   ne bouge pas, personne n'est notifié pour rien) ;
3. mes **commentaires** (ceux que j'ai écrits, modifiés, résolus ou supprimés) ;
4. la **structure** et ses fichiers (sons, transitions…) si je tiens le jeton ;
5. enfin ma fiche (la `revision` de ce que je viens d'écrire).

Dans cet ordre : un fichier ne cite jamais ce qui n'est pas encore écrit. **Rien n'est jamais effacé** (pas de nettoyage de
`dessins/` : un plan supprimé laisse ses dessins ; « Nettoyer le dossier » viendra, réservé au jeton).

**Garde-fou.** Si le cliché montre que j'ai changé un plan (ou le montage) **que je ne tiens pas**, ces changements ne sont écrits
nulle part dans le dossier officiel : ils vont dans `partage/brouillons/` et la barre d'état le dit (« Tu as modifié le plan 12 sans
le tenir : ton travail est mis de côté »). Même chose juste avant qu'un rafraîchissement remplace un plan que j'avais modifié
sans le tenir. Les barrières de l'interface (§11) empêchent d'en arriver là ; le garde-fou est pour celles qu'on aurait oubliées.

**Annuler.** L'historique reste **local**. Quand un rafraîchissement remplace un plan, l'historique n'est pas vidé ; mais pour
qu'annuler ne ressuscite pas un vieux plan de Léa, après chaque annulation / rétablissement les plans que je ne tiens pas (et le
montage, si je n'ai pas le jeton) sont **remis dans leur dernier état reçu** (`rafraichir_etranger`). Perdre un plan (§6) vide
l'historique (une seule fois, avec un message).

**Fermer / quitter** passe par la boîte « Modifications non enregistrées » existante, puis rend ce qu'on tient. Un plantage laisse les
baux expirer en 10 min.

## 11. L'interface en lecture seule

Les barrières existantes du storyboard verrouillé deviennent celles du partage :

- `content_refuses()` (dessin, calques, expositions, effets, caméra, textes du plan) : le plan courant doit être à moi, sinon le
  message dit à qui il est (« Le plan 12 est à Léa. Tu peux le regarder, le jouer et le commenter. ») ;
- `storyboard_refuses()` (ordre, nombre, durée, noms, son, transitions…) : il faut le jeton ; `toggle_mark` (marque d'image) est
  du contenu de plan, pas du montage ;
- détruire, fusionner ou couper un plan demande en plus que le plan soit à moi ou libre (pris au passage).

Tout le reste (lire, jouer, naviguer, commenter, comparer, exporter la copie locale) marche sur n'importe quel plan.

## 12. Ce qui se passe dans les autres fonctions

| Fonction | Dans un projet partagé |
|---|---|
| **Onglets** | un onglet peut être un projet partagé ; chaque onglet a sa session (`EtatProjet`) ; un onglet caché garde son fil (le battement continue) mais n'applique rien tant qu'il n'est pas affiché |
| **Envois** | « Marquer un envoi » demande le jeton ; l'état des plans est celui que je vois (le message dit combien de plans sont « en attente » ou périmés) |
| **Versions de plans** | font partie du plan (son porteur) ; c'est aussi par là que les brouillons et copies en conflit deviennent des versions |
| **Variantes de séquence** (`variantes.rs`) | créer, passer, supprimer ou renommer une variante demande **le jeton du montage** et que les plans qui sortent du film soient à moi ou libres ; les variantes rangées sont une **annexe du montage** (`variantes.json` à côté de la structure, avec leurs dessins dans `dessins/`), écrite par le porteur du jeton — un fichier par projet, pas par plan, parce qu'une variante rangée n'est le plan de personne. Les autres postes les relisent avec la structure. Si des dessins de variantes manquaient à la lecture, le fichier n'est jamais réécrit et les gestes sont refusés. Les plans qui reviennent dans le film sont à moi dès leur arrivée (comme après un « annuler » de suppression) |
| **Relecture** (import de notes) | devient des commentaires de l'équipier qui importe |
| **Export pour le montage, fiche, relecture exportée** | travaillent sur la copie locale ; la fenêtre Équipe dit si des plans sont encore en synchronisation. (Un autre chantier déplace ces exports hors de l'interface ; ils lisent un cliché du projet et n'ont rien à changer ici.) |
| **Copie de secours** | locale, projet classique complet ; rouverte après un plantage elle est un projet **à part** |
| **Importer des plans d'un autre projet** | colle dans les plans libres / crée des plans neufs (jeton) |

## 13. Partager un projet existant

« Fichier > Partager ce projet… » : on choisit un dossier (le dossier Dropbox / Drive / réseau), SHOT y crée `<Nom>/` en
disposition partagée à partir du projet **ouvert** (l'original n'est pas touché) : tous les dessins, sons, modèles, références,
envois ; un fichier de plan par plan ; la structure ; les commentaires au nom de l'équipier. Rien n'est pris : le nouveau projet
s'ouvre dans un **nouvel onglet**, tous les plans et le jeton sont libres, et chacun prend ce qu'il travaille.
À l'inverse, un projet partagé s'enregistre sous un autre nom en projet classique avec « Enregistrer sous… » (copie
complète à plat, utile pour sortir un film du dossier partagé).

## 14. Le serveur plus tard

Pour un `ServeurShot` il faudra : mêmes méthodes de `Depot` par le réseau (HTTP/WebSocket), prise **atomique côté serveur**
(plus de course §6 : la fiche devient un verrou transactionnel ; le protocole des fiches reste le repli), curseur de
changements poussé au lieu de balayé, dessins téléchargés dans un dossier local de travail, authentification. L'interface et
la session n'ont pas à changer : elles ne voient que `Depot`. Voir « Reste à faire ».

## 15. Zéro latence

Toute entrée-sortie du dossier partagé est sur le fil de fond de la session ou sur le fil d'écriture (`SaveJob`). L'interface :
lit un instantané `Arc<Vue>` (un verrou pris une fois par image), reçoit des événements prêts à poser, et ne calcule rien
d'autre. Au repos (rien ne change dans le dossier), la session ne demande **aucune** repeinture. Mesures : voir `docs/PERFORMANCE.md`
(partie « Coopération »).

## 16. Réglages

Préférences > Équipe : mon nom (celui des commentaires par défaut), ma couleur, la durée du bail (10 min), l'intervalle de
balayage (3 s), la prise automatique, les notifications. L'identifiant d'équipier est tiré au hasard à la première
utilisation et gardé dans `reglages.toml` ; il ne quitte jamais le dossier partagé.

---

## État de la réalisation

### Fait (branche `cooperatif`, jamais fusionnée dans `main`)

| Morceau | Fichier | Rôle |
|---|---|---|
| Format partagé | `src/projet_partage.rs` (sous-module de `project.rs`) | structure au format 34 sans plans, un fichier par plan avec ses empreintes de dessins, ouverture (`ouvrir`, `charger_plan`, `charger_structure`), enregistrement de ce qu'on tient (`preparer`, `run_partage`), « Partager ce projet » (`preparer_partage`) |
| Réutilisation du projet classique | `src/project.rs` | `load_manifest_avec(…, Annexes)` : la même lecture pour un projet entier, une structure ou un plan seul ; `dessins_a_ecrire` extrait de `prepare_save` ; `VERSION` reste 33 |
| Numéros | `src/document.rs` | `alloc` préfixé par équipier (`préfixe · 2³² + compteur`), `set_id_prefix`, `reserve_id` qui ne compte que son préfixe |
| Le dépôt | `src/depot.rs` | trait `Depot` (la couture du futur serveur), `DossierPartage`, balayage par curseur (date et taille), repérage des copies en conflit, fichiers temporaires ignorés, réécriture avec reprises |
| Les prises | `src/verrous.rs` | pur : fiches, titulaire (la plus ancienne prise vivante), bail, reprise (`remplace`), course |
| L'équipier | `src/equipe.rs` | identité, séance, couleur, initiales, réglages (`reglages.toml`, section `equipe`) |
| Les commentaires | `src/partagecom.rs` | un fichier par auteur, enregistrements datés, fusion « le plus récent gagne » indépendante de l'ordre |
| La session | `src/cooperation.rs` | `Moteur` (fil de fond : balayage, fiches, lecture des plans, bail, vue publiée) et `Session` (interface : prises, pose des plans, structure, commentaires, brouillons, annuler) |
| L'interface | `src/cooperationui.rs` | barrières, pastilles, fenêtre Équipe, partager, préférences, menus, barre d'état |
| Branchements | `app.rs`, `onglets.rs`, `ouverture.rs`, `enregistrement.rs`, `projetui.rs`, `storyboard.rs`, `boardui.rs`, `rognageui.rs`, `panels.rs`, `prefsui.rs`, `keymap.rs` + `keymap.toml` | voir `docs/ARCHITECTURE.md` |

**Format.** `project::VERSION` = **33** (inchangé). `VERSION_PARTAGE` = **34**, écrite par un projet partagé et lui seul : un SHOT sans le
partage le refuse (« créé par une version plus récente »). Un projet classique s'écrit et se lit comme avant ; aucun fichier d'un
projet classique n'est migré ou touché (essai `sharing_leaves_the_original_untouched_and_a_classic_project_keeps_its_format`).
Nouveaux fichiers : `plans/<n°>.json` (`shot-plan` 1), `commentaires/<équipier>.json` (`shot-commentaires-partage` 1),
`partage/equipe/<équipier>.<séance>.json` (`shot-equipier` 1), `partage/membres/<équipier>.json` (`shot-membre` 1),
`partage/brouillons/…`, `partage/importes/…`.

**Essais** : 62 nouveaux (+1 mesure `#[ignore]`) — `cooperation_tests` (deux postes sur un dossier, dossiers synchronisés imités,
hors connexion), `cooperation_ui_tests` (barrières, enregistrement par la voie ordinaire, onglets, repos, partager, réglages),
`cooperation_modele_tests` (prises pures, noms de fichiers, balayage, équipier, fil de fond pour de vrai), `cooperation_mesures_tests`.
Ce qu'ils prouvent, en une ligne chacun : voir `docs/FIABILITE.md`, partie 13.

### Mesures (poste de développement, dossier local, film de 600 plans dont un de 96 dessins)

| Quoi | Où | Coût |
|---|---|---|
| balayage du dossier, rien de changé | fil de fond | 1,4 ms (premier : 1,7 ms) |
| un tour du moteur, rien de neuf | fil de fond | 1,7 ms |
| `Session::sonder` au repos | interface | 0,04 µs par image d'écran |
| lire et décoder le plan de 96 dessins | fil de fond | 110 ms |
| poser ce plan dans le document | **interface** | 0,07 ms |
| relire la structure de 600 plans / la poser | fil / **interface** | 3,0 ms / 0,5 ms |
| cliché d'un enregistrement (50 plans tenus sur 600) | **interface** | 2,3 ms |
| écrire ces 50 plans | fil d'écriture | 270 ms |
| enregistrer un plan de 96 dessins dont un a changé | fil d'écriture | 12 ms |
| ouvrir le film partagé de 600 plans / le créer à partir d'un projet | fil d'ouverture / fil | 2,1 s / 4,3 s |

Au repos le fil ne demande **aucune** repeinture (essai `an_idle_application_with_a_shared_project_open_asks_for_no_repaint`
et `the_thread_never_asks_for_a_repaint_when_nothing_changes`).

### Limites connues (dites, pas cachées)

- **Deux porteurs du même plan pendant plus de huit secondes** (synchronisation plus lente que la confirmation) peuvent écrire le
  même `dessins/<n°>.png`. Le client de synchronisation en fait deux copies ; le plan reste « en attente » chez les autres jusqu'au
  prochain enregistrement du vrai porteur. Rien n'est perdu — le perdant apprend sa perte, son travail est en brouillon sous des
  numéros neufs — mais les « copies en conflit » de dessins restent dans le dossier (pas de nettoyage automatique).
- Les horloges des postes ne sont pas synchronisées : l'ordre des prises est le même partout, mais « le plus ancien » est celui que
  disent les horloges. Un poste dont l'heure est très fausse gagne ou perd toujours ses courses.
- **Le montage perdu** (jeton pris avant nous) est mis de côté en brouillon mais ne s'importe pas encore : c'est un fichier à lire.
- Importer un brouillon ou une copie en conflit lit les fichiers sur l'interface (un plan, une action de l'utilisateur).
- Les copies en conflit « ignorées » reviennent au prochain lancement (la liste n'est pas retenue).
- Importer un son, marquer un envoi, nommer une séquence, ajouter un personnage… demandent le jeton
  (c'est du montage).
- Variantes de séquence : chaque changement de structure fait relire (sur un fil de fond) les dessins de toutes les variantes rangées ; un plan rangé puis
  revenu garde son ancien fichier de plan (révision repartie de zéro, comme un plan ramené par « annuler »).
- Pas d'enregistrement automatique dans le dossier partagé : les autres ne voient votre travail qu'à Ctrl+S (ou « Rendre »).
- Les exports (montage, fiche, relecture) travaillent sur la copie locale : un plan encore « en attente » y est vide. La fenêtre
  Équipe le dit.
- Un dossier partagé en lecture seule ne laisse pas rejoindre le film : le projet s'ouvre en lecture seule, son chemin n'est pas
  retenu (« Enregistrer sous » en fait une copie à soi).

### Reste à faire

1. **`ServeurShot`** (voir §14) : une réalisation de `Depot` par le réseau avec prise atomique côté serveur, curseur de changements
   poussé, dessins téléchargés dans un dossier local ; authentification ; rôles (aujourd'hui tout le monde peut tout prendre).
2. Enregistrement automatique dans le dossier partagé, et enregistrement automatique d'une prise qui vient d'être confirmée.
3. Nettoyer le dossier partagé (plans orphelins, anciens dessins, copies en conflit de dessins) — réservé au jeton.
4. Importer un montage perdu ; fusionner automatiquement les montages.
5. Persister la liste des copies en conflit ignorées.
6. Présence plus fine (curseur sur la timeline) si le serveur le permet.

### À essayer à l'écran (liste pour Anthony)

**Avec deux fenêtres de SHOT sur un seul poste** (le plus simple pour commencer) :

1. Dans un projet ouvert, **Fichier > Partager ce projet…** : choisir un dossier **Dropbox** ou **Google Drive** (ou un dossier
   quelconque pour essayer). Une fenêtre « Partage » avance, puis le film s'ouvre dans un nouvel onglet. L'original est intact.
2. Lancer une **deuxième fenêtre** de `shot.exe` et ouvrir le même fichier `.pelure` du dossier partagé (deux fenêtres du même
   poste sont deux séances : elles se disputent les plans comme deux personnes). Barre d'état : une icône « équipe » et un chiffre.
3. Fenêtre 1 : clic droit sur le plan 2 dans le montage > **Prendre ce plan**. Une pastille de ta couleur avec tes initiales
   apparaît en haut à gauche du bloc. Fenêtre 2 (quelques secondes après) : la pastille de l'autre est là ; la survoler une seconde
   dit « … tient ce plan ». Essayer de dessiner dedans : refusé, la barre d'état dit à qui il est.
4. Fenêtre 1 : dessiner, **Ctrl+S**. Fenêtre 2 : le dessin arrive tout seul en quelques secondes (barre d'état : « … a enregistré le
   plan 2 »), sans que rien ne bouge sous la main.
5. Fenêtre 1 : clic droit > **Rendre ce plan** (ou fermer la fenêtre). Fenêtre 2 : **Prendre ce plan** puis continuer.
6. **Le montage** : créer un plan, déplacer un plan, changer une durée dans la fenêtre 2 sans avoir pris le montage : refusé
   (« il faut le jeton du montage »). Clic droit > **Prendre le montage**, recommencer : permis ; l'autre fenêtre voit le nouveau plan.
7. **Fichier > Équipe…** : qui est en ligne, ce qu'il tient, les nouveautés.
8. **Commentaires** : sur un plan que personne ne tient, ajouter un commentaire dans une fenêtre : il apparaît dans l'autre.
9. **Plantage** : fermer brutalement une fenêtre (gestionnaire des tâches) qui tient un plan ; dans l'autre, dix minutes plus tard (ou
   en réglant Préférences > Équipe > « Une prise périme après » à 2 minutes), « Reprendre ce plan » demande confirmation.
10. **La course** : prendre le même plan dans les deux fenêtres presque en même temps (avant la pastille de l'autre) : l'une
    perd, le dit, et un **brouillon** est écrit (Équipe > Brouillons > « Importer comme version » dans la fenêtre gagnante).

**Avec deux postes** : pareil, avec le même dossier Dropbox / Drive / NAS. À regarder en plus :

- Dropbox / Drive : l'icône de synchronisation de chaque poste ; **attendre « à jour »** avant de dire « j'ai enregistré ». Réglages
  conseillés : le dossier du film en **« Toujours conserver sur cet appareil »** (Drive : « Disponible hors connexion »), sinon un
  fichier « en ligne seulement » n'est pas lisible tout de suite.
- Couper le réseau d'un poste : la barre d'état passe en rouge (« le dossier partagé ne répond pas »), on continue sur ses plans ;
  au retour tout se rattrape. Couper plus de 10 minutes pendant que l'autre reprend le plan : au retour, le travail est en brouillon.
- Chercher dans le dossier du film des fichiers « … (Copie en conflit de … ) » : la fenêtre Équipe doit les montrer.
- Vérifier qu'un SHOT sans partage (une ancienne version) **refuse** d'ouvrir le `.pelure` du film partagé avec un message clair.