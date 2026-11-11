# Journal de Claude

Ce que Claude a fait ou proposé, par session, le plus récent en haut. Une session = une date.
Pour le détail technique, voir l'historique git et docs/INTERFACE.md (une partie numérotée par chantier).

## 2026-11-11

- Calques dans la timeline, suite : ses deux idées (« trois calques maximum affichés », ceux du dessus et du dessous venant quand on
  monte ou descend ; la rangée des plans trop épaisse) numérotées 39 et 40 et maquettées sur la page des propositions. Avis donné :
  oui aux deux ; le calque actif toujours au milieu, deux fines bandes qui comptent les autres, un bouton « Toute la pile » ; la
  hauteur des plans réglable en tirant son bord, jusqu'au bandeau, retenue par onglet (`montage_rows` donne aujourd'hui aux plans la
  place qui reste, entre 56 et 170 points). La 39 remplace la 37. Rien de codé : attend son accord.
- Ses décisions après avoir vu la maquette : « Toute la pile » gardée (comparer deux calques éloignés, en choisir plusieurs) ; plans
  en taille moyenne ou en bandeau seulement ; l'affichage des calques réglable depuis la barre du bas de la timeline et depuis les
  Préférences ; **un dossier n'est jamais traité comme un calque dans la timeline : toujours plus petit, il se plie et se déplie** ;
  les touches de zoom de la timeline sont celles de la zone de dessin (41 : chez lui Maj+1 et Maj+2 ; la timeline a aujourd'hui
  `bande.zoom_*` sur Alt+1 et Alt+2). Maquette refaite (version 3 de la page) : fenêtre de trois calques avec le calque actif au
  milieu, ligne fine de dossier qui prend sa place à la fine bande, barre du bas, Préférences. Mes choix signalés comme à
  contredire : les flèches sautent les dossiers, boutons de plans réduits à « Nouveau plan », réglages et « ⋯ », un seul réglage
  pour Storyboard et Montage, fin du réglage « Calques non actifs ». Toujours rien de codé.
- Troisième tour : sa règle, à la place de mon « dossier fermé traversé d'un pas » : **un dossier fermé se traverse avec « calque du
  dessus / du dessous » et se referme quand on en sort**. Sa question : d'accord pour les couleurs par dossier, mais risque de
  confusion avec les couleurs par type de calque (partie 98). Réponse proposée et maquettée (version 4 de la page) : jamais les deux
  sens au même endroit ; la bande garde la couleur du type ; le dossier a un filet de couleur dans la colonne de gauche, pris dans
  les treize groupes de couleur existants (`groups.rs`, `Layer::group`, déjà peints en filet dans la timeline de l'Animation, jamais
  utilisés dans ses projets), attribué tout seul à la création du dossier. Attend son avis et son accord pour coder.
- Sur son « code, pas besoin de version portable jusqu'à nouvel ordre » : **les calques dans la timeline** (partie 137, branche
  `calques-timeline`, chantier `Chantiers\calques-timeline`, depuis main 8da4f1d). Trois calques à la fois (le calque actif au milieu,
  toujours à la même place ; deux fines bandes comptent les autres) ou toute la pile ; un dossier est une ligne fine qui se ferme
  et s'ouvre, jamais un calque ; un dossier fermé se traverse avec « calque du dessus / du dessous » et se referme à la sortie,
  sans rien écrire ; la bande garde la couleur du type, le dossier a son filet à gauche (groupes de couleur, d'office par
  dossier) ; plans de hauteur moyenne ou en bandeau ; réglages dans la barre du bas de la bande et dans Préférences > Montage ;
  les touches du zoom de l'image zooment la bande quand le pointeur est dessus. Annoncé puis corrigé auprès de lui : la bande du
  bas ne rétrécit pas toute seule (sa hauteur est le bord tiré à la main), le chantier ne rend donc pas de place à l'image. Trouvé
  en passant : la fenêtre des Préférences s'élargissait de 9 points par image pendant une recherche (défaut d'avant, corrigé) ;
  `ahead_capacity` s'arrêtait net sur une feuille de moins de 64 Ko (corrigé). Un travers du banc d'essai noté : le premier clic
  après l'apparition du pointeur se perd. Captures sur une copie de « retakes brawlydays ». Vérificateur sur la branche : 4 444
  essais réussis, 0 échec, aucun avertissement. Pas fait : les propositions 34, 35, 36 et 38 ; la bande qui prendrait la hauteur
  de son contenu. Pas fusionné : attend son essai ou son accord.
- Fusion de `calques-timeline` dans main (c410030, sans avance rapide) sur son « Fusionne directement », sans qu'il ait essayé l'exe
  de la branche ; pas de version portable (sa consigne du jour). `main` n'avait pas bougé depuis le départ de la branche : aucun
  conflit, même contenu que la branche. L'autre session prévenue avant (message déposé, pas de réponse attendue) ; son chantier
  `cles-glisser` (`Chantiers\multi-calques`, non validé) touche les mêmes fichiers et devra ramener main avant d'être fusionné.
  Vérificateur sur main : 4 444 essais réussis, 0 échec, aucun avertissement, tous les groupes de fiabilité OK (712 s). Poussé sur
  GitHub. Exe principal reconstruit (`target\release\feanor.exe`, 11 h 53) et ouvert deux fois sur une copie de « retakes brawlydays »
  avec de faux réglages : pas de plantage. Pas vérifié : la timeline manipulée à la main dans le vrai exe (sa liste 137.8). Le
  dossier de chantier `Chantiers\calques-timeline` attend son accord pour être retiré.
- Son essai de l'exe de midi, quatre retours : « Ajoute les bandes de couleur du dossier dans le panneau calque également » ;
  « Modifier la durée des images clés et la durée des plans n'est pas intégré » ; « Sélectionner plusieurs calques et créer une
  nouvelle image clé (vides également) n'a pas été intégré » ; « Déplacer une image clé avant ou après la suivante n'a pas été
  intégré ». Ce que j'ai trouvé : la durée des clés et des plans et les clés glissées avant ou après une autre étaient codées par
  une autre session depuis le 10 au soir, mais restées **non validées** dans `Chantiers\multi-calques` (branche `cles-glisser`,
  16 fichiers modifiés, 3 neufs, dernière écriture le 11 à 0 h 03), jamais vérifiées ni fusionnées ; aucune session ne s'en réclame
  (l'une hors ligne, l'autre sans chantier en cours). Repris : validé tel que trouvé (4bc6fbf), main ramené dedans (d8aaf8f ; un
  seul conflit, `docs/INTERFACE.md` : sa partie devient la 138), ses 16 essais passent. La nouvelle clé sur plusieurs calques, elle,
  **était dans main** (partie 132) mais trois gestes courants la réduisaient à un seul calque : un clic dans la piste d'un des
  calques choisis (le calque courant n'était pas inscrit dans le choix), des clés prises sur plusieurs pistes sans cliquer de nom,
  et le double-clic sur une piste (qui défaisait en plus le choix) ; corrigés (partie 137.11). Son geste exact n'est pas connu : son
  compteur d'usage dit seulement F5 au clavier. Le filet du dossier dans le panneau Calques : tout à gauche du rang, la bande de la
  sorte juste après, rien de changé dans un plan sans dossier ; regardé sur une capture d'une copie de « retakes brawlydays ».
  Vérificateur sur la branche : 4 464 essais réussis, 0 échec, aucun avertissement (669 s). Sur son second « Fusionne directement »
  (donné pendant le travail) : fusion dans main à suivre.
- Fusion de `cles-glisser` dans main (805b8e3, sans avance rapide) : `main` n'avait pas bougé, même contenu que la branche vérifiée
  (le vérificateur n'a donc pas été relancé sur main). L'autre session prévenue par deux messages, sans réponse. Poussé sur GitHub.
  Exe principal reconstruit (`target\release\feanor.exe`, 13 h 54) et ouvert deux fois sur une copie de « retakes brawlydays » avec
  de faux réglages : pas de plantage. Pas de version portable. Pas vérifié : aucun geste fait à la main dans le vrai exe (ses listes
  137.11 et 138) ; la partie 138 n'a été ni écrite ni essayée par moi au-delà de ses 16 essais et du vérificateur. Les dossiers de
  chantier `Chantiers\calques-timeline` et `Chantiers\multi-calques` attendent son accord pour être retirés.
- Sur son « Oui, retire-les » : les deux dossiers de chantier retirés (propres, leurs branches entièrement dans main ; 21 Go de
  fichiers de construction rendus au disque). Les branches `calques-timeline` (7e89e7b) et `cles-glisser` (49472b8) restent. Il ne
  reste que `Source`.
- Deux nouveaux retours de son essai de l'exe de 13 h 54 (partie 139, branche `dossier-glisser`, chantier
  `Chantiers\dossier-glisser`, depuis main 8c2417d ; le dossier de construction est celui de `Source`, pour ne pas tout
  recompiler). **« glisser des calques dans un dossier ne fonctionne pas »** (capture : trois calques choisis sous un dossier vide) :
  quatre causes dans le code — pris par son nom, un calque ne partait pas du panneau Calques (le texte du nom se sélectionnait) ; le
  lâcher sur le rang d'un dossier ne rangeait pas dedans ; le calque posé juste sous un dossier vide ne pouvait pas y entrer ; un
  seul calque partait, et rien n'entrait dans un dossier fermé. Corrigé aux trois endroits (panneau Calques, colonne de la bande,
  en-têtes de l'Animation) : lâchés sur la ligne d'un dossier, les calques choisis y entrent ensemble (`folders::poser`,
  `layerheads::cible_du_glisse`). **« le ctrl+c/ctrl+v ne fonctionne plus pour copier/coller plusieurs lignes vectorielles »** :
  avec deux clés restées choisies dans la timeline (ce que laisse une nouvelle clé posée sur deux calques) et un seul calque choisi,
  la copie partait dans le presse-papiers « plusieurs dessins » que Ctrl+V ne lisait qu'avec deux calques choisis : rien ne se
  collait. Ctrl+V colle maintenant la dernière copie quel que soit le nombre de calques choisis, et un clic sur le nom d'un calque
  lâche les clés choisies sur les autres. Son geste exact n'est pas connu : c'est le seul chemin trouvé où le collage ne fait rien,
  reproduit par un essai (écrit après la correction : je ne l'ai pas vu échouer avant). Vérificateur sur la branche : 4 472 essais
  réussis, 0 échec, aucun avertissement. Exe de la branche (`Source\target\msvc\release\feanor.exe`, 14 h 41) ouvert deux fois sur
  une copie de « retakes brawlydays » avec de faux réglages : pas de plantage. Pas fusionné : attend son accord.
- Fusion de `dossier-glisser` dans main (b78ea38, sans avance rapide) sur son « Fusionne directement » : `main` n'avait pas bougé,
  même contenu que la branche vérifiée (vérificateur non relancé sur main). L'autre session prévenue (message déposé, sans réponse).
  Poussé sur GitHub. Son FEANOR, ouvert depuis 13 h 59, fermé sans enregistrer pour reconstruire (il était prévenu ; l'écriture est
  continue). Exe principal reconstruit (`target\release\feanor.exe`, 14 h 48) et ouvert deux fois sur une copie de « retakes
  brawlydays » avec de faux réglages : pas de plantage. Pas de version portable. Pas vérifié : aucun geste fait à la main dans le
  vrai exe (sa liste 139.3). Le dossier de chantier `Chantiers\dossier-glisser` attend son accord pour être retiré.
- Sur son « Oui, retire-le » : le dossier de chantier `Chantiers\dossier-glisser` retiré (propre, sa branche entièrement dans main).
  La branche `dossier-glisser` (0293edd) reste. Il ne reste que `Source`.
- Nouveau retour, deux captures à l'appui : « Sélectionner 3 calques et cliquer sur F5 ne crée une clé d'image que sur un seul
  calque au lieu des trois ». Sur ses captures, les pistes de deux des trois calques choisis sont ternes : c'est ainsi que la bande
  peint un calque masqué. Son projet le confirme (lu sans y toucher) : « Vectoriel 14 » et « Vectoriel 15 » ont l'œil fermé. La
  nouvelle clé sur plusieurs calques (partie 132) laissait de côté les calques masqués, en le disant seulement dans la barre
  d'état, alors qu'un calque masqué seul la reçoit. Corrigé (partie 139.5, branche `cle-masques`, chantier `Chantiers\cle-masques`,
  depuis main 4395c46) : un calque masqué reçoit sa clé ; les calques verrouillés restent de côté (à décider par lui). Vérifié sur
  une copie fraîche de son projet : les trois calques de ses captures choisis, F5, « Nouvelle clé sur 3 calques ». Vérificateur sur
  la branche : 4 473 essais réussis, 0 échec, aucun avertissement. Exe de la branche (`Source\target\msvc\release\feanor.exe`,
  15 h 59) ouvert deux fois sur une copie de projet avec de faux réglages : pas de plantage. Pas fusionné : attend son accord.
  Ma correction du matin (partie 137.11) visait d'autres gestes : elle n'était pas la cause de son cas.
- Fusion de `cle-masques` dans main (02c1ceb, sans avance rapide) sur son « Fusionne directement » : `main` n'avait pas bougé, même
  contenu que la branche vérifiée. L'autre session prévenue (message déposé, sans réponse). Poussé sur GitHub. Son FEANOR, ouvert
  depuis 15 h 35, fermé sans enregistrer pour reconstruire (il était prévenu). Exe principal reconstruit (`target\release\feanor.exe`,
  16 h 05) et ouvert deux fois sur une copie de projet avec de faux réglages : pas de plantage. Pas de version portable. Sa question
  ouverte : la clé sur les calques verrouillés. Le dossier de chantier `Chantiers\cle-masques` attend son accord pour être retiré.
- Sa demande, capture à l'appui (la bande tirée très haut, à moitié vide) : « que la taille des calques dans la timeline s'adapte
  aussi à la taille du panneau. La taille la plus grande reste la même mais quand il y a plus de place […] les calques les plus
  proches du calque sélectionné prennent aussi la taille la plus grande », puis « il faut que ce soit toujours le plus confortable
  possible à utiliser ». Codé (partie 140, branche `calques-place`, chantier `Chantiers\calques-place`) : quand la bande est plus
  haute que ses pistes, les calques grandissent un à un autour du calque actif (le dessus, le dessous, puis les suivants) tant
  qu'il reste de la place ; dans « Trois calques », les deux voisins ; pas assez de place : rien ne grandit, la bande défile comme
  avant. Une case dans Préférences > Montage, allumée d'office (éteinte dans les essais). Captures sur une copie fraîche de son
  projet, regardées et envoyées. Vérificateur sur la branche : 4 478 essais réussis, 0 échec, aucun avertissement. Exe de la
  branche (16 h 27) ouvert deux fois sur une copie de projet avec de faux réglages : pas de plantage. Son « Fusionne directement »
  est arrivé pendant le vérificateur : fusion dans main à suivre. Limite dite : deux hauteurs seulement (grande ou en bande).

## 2026-11-10

- Fusion de `camera-souvient` dans main (35a6e42, sans avance rapide), après l'essai d'Anthony dans la version portable du 9 novembre et
  son accord (« ok tu peux fusionner avec le reste ») ; l'autre session prévenue avant. `main` n'avait pas bougé depuis le départ de la
  branche : aucun conflit. Vérificateur sur main : 4 329 essais réussis, 0 échec, aucun avertissement, tous les groupes de fiabilité OK
  (828 s). Poussé sur GitHub. Exe principal reconstruit (`target\release\feanor.exe`) et ouvert sur une copie de projet avec de faux
  réglages : pas de plantage. Le dossier de chantier `Chantiers\camera-souvient` attend son accord pour être retiré.
- Son retour sur « afa » (« la planche ne prend plus le trait… le trait disparaît au lâcher ») : la cause était l'interrupteur « Peint
  sur l'aplat » resté allumé (vu dans sa vidéo d'écran), pas la fusion ; ses trois symptômes rejoués sur une copie de son projet. Sur son
  « Oui, corrige » : partie 133, branche `souvient-retours` (chantier `Chantiers\souvient-retours`, depuis main 3e2b090). Un trait posé
  sur le papier va au papier ; un trait qui ne laisse rien ne crée ni aplat, ni dessin vide, ni étape, ni plan vide ; la barre d'état
  dit pourquoi ; la barre du bas de la planche montre l'interrupteur allumé, un clic l'éteint. Vérificateur sur la branche : 4 339
  essais réussis, 0 échec, aucun avertissement, fiabilité OK. Exe de branche ouvert sur une copie de son projet avec de faux réglages :
  pas de plantage. Pas fusionné : attend son essai et son accord.
- L'autre session a fusionné `multi-calques` dans main (00a047e, partie 132) et l'a demandé : main fusionné dans `souvient-retours`
  (c7addc7), seul conflit dans docs/INTERFACE.md (132 puis 133, les deux gardées). Vérificateur : un premier passage sous forte charge
  (1 010 s) a vu échouer `camera_traits_tests::scrubbing_far_away_never_cuts_the_stroke_again` (aperçu réduit des effets fait en
  arrière-plan, sensible au temps) ; seul, il passe trois fois sur trois ; second passage complet : 4 385 réussis, 0 échec, aucun
  avertissement, fiabilité OK (416 s). Exe de branche reconstruit et ouvert sur une copie d'afa : pas de plantage. Ctrl+A des Vignettes
  laissé à l'autre session (vignettesel.rs, partie 134).
- Sur son « Fusionne directement » : `souvient-retours` fusionné dans main (cc9e505, sans avance rapide), l'autre session prévenue
  avant ; main n'avait pas bougé depuis 00a047e : aucun conflit. Vérificateur sur main : 4 385 essais réussis, 0 échec, aucun
  avertissement, fiabilité OK (985 s). Poussé sur GitHub. Exe principal reconstruit (`target\release\feanor.exe`, FEANOR n'était pas
  ouvert) et ouvert sur une copie d'afa avec de faux réglages : pas de plantage. Le dossier `Chantiers\souvient-retours` retiré avec
  son accord (« Oui, retire-le ») ; la branche est gardée.
- Audit à sa demande (« audit sévère s'il faut »), à partir de son compteur d'habitudes et de ses projets : `suivi/AUDIT-2026-11-10.md`,
  cinq constats, propositions 21 à 29, puis la 30 (« La feuille par axe ») en réponse à « si tu repartais de zéro ». Rien de codé.
- Sur son « fais la proposition 25 » : **on n'enregistre plus** (partie 135, branche `sans-enregistrer`, chantier
  `Chantiers\sans-enregistrer`, depuis main a17ad33). FEANOR écrit le projet tout seul dès que le stylet se lève (l'enregistrement
  de toujours, déclenché par `continu.rs`, silencieux) ; fermer et quitter ne demandent plus rien ; Ctrl+S pose un jalon ; un jalon
  automatique à chaque ouverture remplace « Ne pas enregistrer » ; Fichier > Jalons… pour nommer, revenir, supprimer. Un jalon est
  un projet complet en liens physiques (`jalons.rs`), qui ne prend presque pas de place. Pas pour un film partagé ni un projet sans
  dossier. Réglage « Enregistrer en continu » dans les Préférences. Le format ne change pas (52). Trouvé en relisant : chaque
  écriture réécrivait les réglages (liste des récents) et le titre de la fenêtre aurait clignoté ; corrigé. Vérificateur sur la
  branche : 4 406 essais réussis, 0 échec, aucun avertissement, fiabilité OK (975 s). Exe de branche lancé sur une copie d'« afa »
  avec de faux réglages : jalon d'ouverture posé en 1,5 s (300 fichiers, en liens), aucun fichier du projet changé, pas de second
  jalon au second lancement, arrêt brutal sans reste. Pas fait : le fichier de projet unique (étape à part), l'annulation qui survit
  à la fermeture. Pas fusionné : attend son essai et son accord.
- L'autre session a fusionné dans main `vignettes-ctrl-a` (d07678e, partie 134) puis `selection-tout-2` (d3662ef, partie 136) et l'a
  rappelé : main fusionné dans `sans-enregistrer` (d7f1761), seul conflit dans docs/INTERFACE.md (parties rangées 134, 135, 136).
  Vérifié dans le code reçu : ni fil, ni canal, ni travail de fond nouveau, donc rien à ajouter à `App::continu_retenu`. Vérificateur
  sur la branche : 4 437 essais réussis, 0 échec, aucun avertissement, fiabilité OK (785 s). L'exe de la branche n'a pas été
  reconstruit : la version portable du soir (dac56c7) reste celle de son essai. Toujours pas fusionné : attend son essai et son accord.
- Sur son « Fusionne directement » : `sans-enregistrer` fusionné dans main (6093d46, sans avance rapide), l'autre session prévenue
  avant ; main n'avait pas bougé depuis d3662ef : aucun conflit, contenu identique à la branche vérifiée. Vérificateur sur main :
  4 437 essais réussis, 0 échec, aucun avertissement, fiabilité OK (732 s). Poussé sur GitHub. Exe principal reconstruit
  (`target\release\feanor.exe`, FEANOR n'était pas ouvert) et ouvert deux fois sur une copie d'afa avec de faux réglages : pas de
  plantage, jalon d'ouverture posé au premier lancement (300 fichiers), aucun fichier du projet changé, pas de second jalon au
  second lancement, arrêt brutal sans reste. Il n'a pas essayé la version portable avant : le stylet pendant l'écriture continue
  reste à juger par lui, dans son FEANOR habituel. Le dossier `Chantiers\sans-enregistrer` retiré avec son accord (« Oui,
  retire-le ») ; la branche est gardée.
- Sa remarque sur les calques dans la timeline (« un peu bordélique », les bandes repliées « manquent parfois de lisibilité ») : relevé
  du code (parties 29 et 68, `pistes.rs`, `montagerows.rs`) et de ses projets en lecture seule (13 calques dans le plan le plus chargé
  de « retakes brawlydays », dix bandes de la même couleur, aucun nom donné par lui, 172 montées et descentes de calque au clavier).
  Huit propositions numérotées 31 à 38 (couleur par dossier, bandes et noms plus lisibles, dossiers visibles, calques cachés en une
  ligne, noms sans les taper, retrouver un calque par son dessin, pile qui ne saute plus, un plan à la fois) et une page de maquettes
  cliquables. Rien de codé : attend son choix par numéros.

## 2026-11-09

- La caméra se souvient (partie 131, branche `camera-souvient`, chantier `Chantiers\camera-souvient`) : à sa demande de repenser les
  Vignettes « quitte à repartir de zéro », cinq approches présentées (A à E) ; son choix : « Ok on essaye A ». **Un trait appartient tout
  entier à la case où le stylet s'est posé ; hors de toute case, il appartient au papier** : plus de découpe du trait au bord des cases.
  La planche montre chaque trait en entier, par-dessus les cases qu'il traverse (« la feuille ») ; le passe-partout montre le film (chaque
  case son plan) ; le plan, le montage et les exports du film restent coupés au cadre. La vignette d'une case s'étend à son encre (quatre
  cadres par côté au plus). La sélection de la planche (vectorielle et bitmap), le collage et la case qui naît reposent le trait entier là
  où il commence ; un vieux trait coupé redevient entier quand on le déplace. Le bout d'un trait ne fait plus naître la case pâle
  suivante ; la marge de la première case pâle la fait naître. La gomme de la planche efface aussi ce que les cases ont dessiné hors de
  leur cadre (`gommedebord.rs`). Le format du projet ne change pas. Mesure demandée par l'autre session (60 cases, 2 040 traits) : une
  image d'écran en 0,89 ms. Onze essais neufs (`camera_souvient_tests.rs`), les essais d'avant réécrits pour dire la règle neuve. Limites
  notées : caméra en perspective (le trait qui dépasse s'amincit), quatre cadres au plus, un débord ne se gomme pas depuis la case d'une
  autre. **Relecture indépendante** du code avant toute proposition de fusion (demandée par l'autre session) : quinze remarques, dont
  plusieurs sérieuses sur la gomme toute neuve (elle prenait un trait resté dans son cadre, ignorait le verrou du storyboard et le
  passe-partout) et sur les dessins montrés par plusieurs cases. Corrigé : ces dessins-là (le Décor, une planche-mère, un calque de toute
  la scène) gardent la règle d'avant, pour ne pas s'étaler autour de chaque case qui les montre ; un trait bitmap parti du papier n'emporte
  plus un double du dessin qu'il traverse ; la marge ordinaire d'une case ne disparaît plus dans un grand export. Pas fusionné : exe de
  branche à essayer par Anthony.
- À sa demande (« fusionner sur une version portable seulement sans altérer la version principale ») : version portable d'essai
  `Logiciel\Portable\FEANOR-portable-2026-11-09-camera-souvient` (dossier et zip), faite de l'exe de la branche (a9f3540). `main` n'avait
  pas bougé depuis le départ de la branche (6f27220) : cet exe est déjà le FEANOR habituel plus la partie 131, sans aucune fusion. Ouverte
  sur une copie de projet avec de faux réglages : pas de plantage. `main`, `Source` et son exe ne sont pas touchés.

## 2026-11-08

- Dépôt GitHub de sauvegarde, à la demande d'Anthony (« afin de ne rien perdre ») : il a créé le dépôt privé
  https://github.com/ferreanthony-glitch/Feanor lui-même ; `origin` est branché dessus dans `Logiciel\Source`, et tout l'historique est
  poussé (main, les 118 branches, le tag `archive/memoire-dessins`). Son premier commit fait en ligne (un README de deux lignes) est repris
  dans l'historique, le README détaillé du dépôt est gardé. La connexion passe par le gestionnaire d'identifiants de Windows, validée par lui
  dans le navigateur ; `gh` (l'outil GitHub en ligne de commande) est installé mais pas connecté. Avant de pousser : aucun secret dans les
  fichiers suivis (`clessecretes.rs` range les clés dans Windows, jamais dans un fichier).
- La case qui naît prend le papier (partie 128, branche `cases-prennent`, chantier `Chantiers\cases-prennent`) : réponse à ses deux captures
  (« les caméras non activées au début cachent le trait quand elles s'activent »). Une case pâle qui devient un plan (un trait dedans, la barre
  de continuité) prend les traits du papier (« Hors cadres ») qui passent dans son champ : la part dans le champ entre dans le dessin du plan,
  le reste reste sur le papier, par la repose de la sélection de la planche à l'identique (`caseprend.rs`, sans toucher aux fichiers de l'autre
  session) ; une seule étape d'annulation avec la création. Suites : les pixels bitmap du papier (après `vignettes-bitmap`), la case née par le
  bout d'un trait (`horschamp_creer_cases`, fichier de l'autre session), un plan bitmap qui naît. Sa réponse sur l'architecture : pas de « planche
  = dessin, case = caméra » (un plan ne doit montrer que ce qui a été dessiné dans son champ, pas toute la planche en baladant la caméra), et le
  Décor reste tel quel. Trouvé en passant et corrigé (`app.rs`, `clamp_nav`) : après Ctrl+Z d'une continuité, l'image courante restait au-delà
  de la fin du plan, et un trait dans la case des Vignettes partait dans une image neuve au bout du plan (qui s'allongeait), la vignette du
  montage restant vide. Quatre essais (`caseprend_tests.rs`). Fusionné dans main (b334410) sur son « Oui, fusionne » après son essai de l'exe
  de branche ; l'exe de main est reconstruit (son FEANOR ouvert arrêté sans enregistrer, comme convenu).
- Fusion dans main (aa03f7f) de `planche-mere`, à la demande d'Anthony (« Oui, fusionne »), après la fusion de `vignettes-selection` par l'autre
  session (6bcade3) : la planche-mère (partie 126, format de projet 51), sans sa partie Vignettes, qui vient dans un chantier suivant. Essai
  ajouté à la demande de l'autre session : un trait d'un dessin partagé montré par deux cases est pris une seule fois, le déplacer le déplace dans
  les deux cases, rien n'est dupliqué (limite : le lasso ne le trouve que dans la première case). Vérificateur sur la branche à jour de main :
  4259 réussis sur 4259, aucun avertissement. `feanor.exe` de main reconstruit ; le FEANOR d'Anthony, ouvert à 17:53, a été fermé sans
  enregistrer pour cela (sa règle). L'autre session fabrique le zip portable sur cet exe.
- La planche-mère (idée 4 d'Anthony ; branche `planche-mere`, chantier `Chantiers\planche-mere`, partie 126 ; conception validée par ses trois
  réponses : tout le plan, Storyboard d'abord, « Réutiliser » dans les commandes et rien de plus dans Aide) : un calque porte une clé de
  partage, ses exemplaires dans plusieurs plans montrent les mêmes dessins, chacun sous la caméra de son plan (`partage.rs`,
  `sync_partages` à chaque image comme les calques de toute la scène) ; « Réutiliser dans… » (plan suivant, précédent, séquence, plans
  choisis) et « Détacher » dans le menu d'un calque, maillon dans sa ligne ; « Réutiliser le plan » (un plan lié dans une scène neuve, même
  dessin, à cadrer) et « Détacher le plan » dans le menu Storyboard > Plans (les menus du montage restent à leur dizaine d'entrées). Couper
  garde le lien, réunir détache le second, dupliquer et coller ne lient pas. Format de projet 51. Fusion dans main après celle de
  `vignettes-selection` (6bcade3), à la demande d'Anthony (« Oui, fusionne »), sans la partie Vignettes (maillon sur les cases, trait entier au
  dessin partagé), qui vient dans un chantier suivant, à coordonner avec l'autre session (ses fichiers).
- Fusion dans main (7c7df04) de `finitions`, à la demande d'Anthony (« Fusionne finitions ») après son essai de la Trajectoire (« parfait ») :
  la trajectoire suit les images insérées ou retirées et peut faire tourner le dessin (partie 119, format 50), la licence de ffmpeg dans
  À propos et les trois touches (partie 124). Vérificateur sur la branche : 4235 réussis sur 4235, aucun avertissement. `feanor.exe` de main
  reconstruit. L'autre session (`vignettes-selection`) est prévenue. La planche-mère (branche `planche-mere`, partie 126) reste à part.
- Finitions (branche `finitions`, chantier `Chantiers\finitions`, à la demande d'Anthony) : la Trajectoire d'abord (partie 119) — **insérer ou
  retirer des images décale la trajectoire** (tout ce qui passe par `Layer::shift_frames` : le départ, la fin et chaque marque restent sur
  leur image, comme les clés et les dessins ; la fin est posée dès le tracé sur la dernière image du plan) et **« Le dessin tourne avec le
  chemin »** (case des réglages : il pivote autour du point d'ancrage en gardant son orientation du départ ; clés d'interpolation : l'angle
  sur chaque clé ; clés d'images : chaque copie tournée, bitmap rééchantillonné par `Raster::transformee`). Format de projet 50. Puis le
  petit lot V1 (partie 124) : la **licence de ffmpeg dans À propos** (ligne LGPL, dépliant avec le texte de la licence, la configuration
  exacte, le ffmpeg du poste, le dossier `ffmpeg-licence` de la version portable, les sources) ; **trois touches** proposées : Ctrl+Maj+D
  duplique le plan, Maj+C l'outil Caméra, Maj+K la clé de caméra ; les versions rangées d'une séquence vidée étaient déjà protégées
  (refus avec la raison, chantier `versions-fiables` du 5 novembre, partie 102) : rien à refaire. Remarque : la version portable du
  8 novembre ne joint pas l'archive des sources de ffmpeg que son `CONFIGURATION.txt` annonce. Non fusionné : attend son essai et son
  « Fusionne ».
- Vignettes : traits coupés autour des cases (capture d'Anthony). Cause : « Peindre derrière » allumé dans ses réglages ; depuis 6d89a00 (2026-10-30) un trait peint derrière n'était pas réparti et restait dans le champ de sa case (formes aussi). Pas une correction perdue en fusion (six fusions vérifiées fichier par fichier). Corrigé (a06d4c5, fusion 90f4db4), essai `vignettes_dessin_tests` rouge puis vert, vérificateur 4228 réussis deux passages. Portable `FEANOR-portable-2026-11-08.zip`.

## 2026-11-07

- Fusion dans main (760cba4) de `duree-cles` : effets au-delà de la feuille (partie 121), traits cachés (122), durée d'un plan et clés alternées (123), planche en tuiles exactes, Annuler sans gel (15 s → 0,2 s sur un gros cas, en essais automatiques), réparation de plusieurs causes d'essais instables. Relectures : gel à l'annulation, décalage à tort sur les anciens projets (marqueur `cles_v2`, essai sur un vrai fichier ancien) et trajectoire qui ne suivait pas Ctrl : réels, corrigés. Vérification finale 4226 réussis, deux passages. Portable `FEANOR-portable-2026-11-07-soir.zip`, ouvert sur une copie de projet. Incident sans perte : un script de renumérotation (alias `Ren` de PowerShell) a vidé 13 fichiers du worktree, restaurés depuis l'index avant commit.
- Fusion dans main (3a6cfc0) à la demande d'Anthony, après deux essais (« ça marche très bien ») : l'outil Trajectoire (partie 119,
  format de projet 49) et les poignées de lissage de la caméra au repos (partie 120). Suite complète sur la branche : 4133 réussis sur
  4134, un essai de temps différent à chaque passage, réussi seul. `feanor.exe` reconstruit. Le chantier `trajectoire` peut partir avec
  son accord.
- Les poignées de lissage de la caméra attendent à côté de chaque clé dès qu'un mouvement existe, au repos (à 0), et se tirent ; ramenées
  près de la clé, elles reviennent au repos (partie 120, branche `trajectoire`). Rien ne change dans le projet.
- L'outil Trajectoire (partie 119, branche `trajectoire`), l'idée d'Anthony : un trait trace le chemin du calque, de petits
  traits en travers posent les marques d'espacement comme sur une feuille de timing, et les clés de position se posent toutes seules, une par
  image, le long de la courbe. Marques glissées ou arrachées, chemin déplacé ou retracé, silhouettes pâles aux jalons, pas des marques (durée
  du plan partagée), détachement quand on retouche une clé à la main. Format de projet 49. Puis, à sa demande : la clé de départ démarre
  sur la tête de lecture, deux sortes de clés (interpolées, ou par images). Après son essai : les « clés d'images » sont de vrais dessins,
  un à chaque marque, copie du dessin du départ déplacée sur le chemin (la timeline se scinde) ; glisser une clé sur la timeline recale la
  trajectoire (fin, départ, temps d'une marque) au lieu de la détacher ; une fin explicite ; tirer sur le chemin le tord comme une corde
  (Maj : en bloc). Puis, « ça marche très bien » : les clés d'images deviennent le réglage d'origine, et « ce qui suit le chemin » se
  choisit (là où part le trait, le bas du dessin, le centre, le haut, un bord, ou un point cliqué). Et la trajectoire ne s'en va plus
  d'elle-même (une clé supprimée la détachait) : Suppr retire une marque, poser ou lisser une clé à la main est refusé, Transformer
  déplace le calque et son chemin ensemble. 25 essais. Reste son essai.
- Idées retenues par Anthony dans les vingt proposées : 1 + 8 (dessiner pendant la lecture, la page de relecture existant déjà), 3, 4, 7, 11,
  14, 15, 16, 18, 20 ; sa trajectoire remplace 13 ; 2, 5, 6, 9, 10 écartées. Rien d'autre n'est codé.
- Fusion dans main (6a06b1c) : plan écarté (partie 115, format 48), « + » de la peau de Maj pour un cadre de plus (116), poignées de lissage
  sur la piste caméra (117), traits coupés au bord de la feuille (118 : les effets, dont le tremblement par défaut, ne rendaient que la feuille).
  Relectures indépendantes : 8 défauts corrigés sur le plan écarté, 3 points corrigés sur les traits. Vérificateur : 4109 réussis, 0 échec, deux passages.
  `feanor.exe` reconstruit, ouvert sur une copie de projet. Rien d'essayé au stylet. Limites : secousse, onde, vignettage et effets du trait
  s'arrêtent au bord de la feuille ; colonne un peu plus claire au raccord sur carte graphique (4,5 % à l'intensité 0,5).
- Incident : un agent a écrit `camera_traits_tests.rs` dans Work\BRAWLSTAR\BRAWLIDAYS\3D par un chemin relatif. Signalé à Anthony, suppression à sa décision.
- Mac : reporté par Anthony tant que la version PC n'est pas peaufinée. Rien n'est envoyé sur GitHub.

- Fusion dans main (9819707) à la demande d'Anthony : le bouton du stylet est le clic droit, le stylet retourné prend la Gomme (partie 114,
  branche `stylet-branche`). `feanor.exe` reconstruit. Reste son essai à la tablette ; l'inclinaison attend.
- Le stylet branché (partie 114, branche `stylet-branche`) : d'après son relevé (bouton vu pointe posée, gomme vue, et « clic droit : 0 »
  côté FEANOR, winit jetant le bouton), le bouton du stylet devient le clic droit (réglages de l'outil, anneau) et le stylet retourné prend
  la Gomme. Réglage dans Préférences > Dessin. L'inclinaison attend.
- Fusion dans main (547b51f) à la demande d'Anthony : Mes habitudes et notes de friction (partie 112), banc du stylet (partie 114), après
  fusion de main dans la branche. Vérificateur sur le résultat : 4003 réussis, 0 échec, aucun avertissement. `feanor.exe` reconstruit et ouvert
  sans erreur sur une copie de TEST PELURE_effets. Reste son essai : F9, Aide > Mes habitudes, et le relevé du stylet à la tablette.

## 2026-11-06

- Le banc du stylet (INTERFACE partie 114, branche `stylet-entier`, posée sur `usage-frictions`, pas fusionnée) : Aide > Ce que le stylet
  transmet… écoute le stylet auprès de Windows (bouton, stylet retourné, gomme, inclinaison, rotation) sans rien changer au dessin, et le
  montre à côté de ce que FEANOR reçoit aujourd'hui. L'écoute n'est posée qu'à l'ouverture du banc. Reste l'essai d'Anthony à la tablette :
  son relevé (`%LOCALAPPDATA%\SHOT\stylet-releve.txt`) décide de ce qu'on branche (gomme au dos, inclinaison, bouton tenu).
- « Tout au stylet » : page de propositions publiée (trois règles, 23 gestes, six décisions à prendre). Rien de codé avant ses réponses.
- Mes habitudes et notes de friction (INTERFACE partie 112, branche `usage-frictions`, pas fusionnée) : un compteur local des commandes par
  chemin (clavier, icône, menu, anneau, palette), la fenêtre Aide > Mes habitudes, et F9 pour noter une friction avec une capture et les
  dernières commandes, dans `Documents\SHOT\Frictions`. Vérificateur : 3985 réussis, aucun avertissement. Reste son essai, puis la fusion.
- Traits continus (partie 111, c11f743) et Vignettes simplifiées par Fable (analyse `suivi/ANALYSE-VIGNETTES.md`, partie 113) fusionnés (f5caddd).
  Vérification complète : 3981 réussis, 0 échec, deux passages, aucun avertissement. Ouvert sans plantage sur une copie. Version FEANOR-portable-2026-11-06c.
  Reste : décision 6, plan annulé, essai au stylet.
- L'anneau sous le stylet est fusionné dans main à la demande d'Anthony (ef983ac) : `feanor.exe` reconstruit, ouvert sans erreur sur une
  copie de TEST PELURE_effets. Le contenu est celui que le vérificateur a passé sur la branche (3954 réussis, aucun avertissement).
  Reste son essai à la tablette ; le dossier de chantier `vignettes-passepartout` (2,4 Go) peut partir avec son accord.
- L'anneau sous le stylet (INTERFACE partie 111, branche `anneau-stylet`) : après la maquette essayée par Anthony au stylet,
  huit commandes autour de la pointe par le bouton du stylet, second anneau des pastilles, onglet Préférences > Anneau (commandes, délai),
  « Envoyer dans l'anneau » au clic droit d'une icône. Joue sur le dessin et sur la planche des Vignettes ; le clic bref garde les réglages
  de l'outil ou le menu de la case. 27 essais, rendu vu sur cinq captures. Non vérifié sans lui : que le bouton de son stylet arrive
  comme un clic droit tenu.
- Fusionnés aussi : passe-partout, numéros, flèche (partie 110) ; corbeille, format 47, gestes de montage sans casse, J/K/L (partie 104, 7a0c6ad).
  Vérification complète au calme, avec le ffmpeg allégé : 3927 réussis, 0 échec, deux passages, aucun avertissement, aucun plantage.
  Une relecture a trouvé deux pertes de dessins dans la corbeille avant fusion, corrigées. Version portable FEANOR-portable-2026-11-06b.
- Fusionnés dans main (7cac5ed, INTERFACE parties 101 à 103 et 105 à 109) : canevas libre et outil Caméra, versions, aplat, textes et séquences,
  brosses dédiées, références, exports à la clé, son par plan, nomenclature, color script. Suite complète : 3866 réussis, 1 échec instable connu
  (`vignettes_retouches_tests`, réussit 3 fois sur 3 seul). Reste : avertissements à corriger, coupe des captures 102 et 103 non reproduite,
  corbeille (format 47) et passe-partout en cours. Version portable FEANOR-portable-2026-11-06.

## 2026-11-05

- Plus d'onglet Animation : double clic sur un plan, bouton « Retour au storyboard » (fusion 9a9c77c, INTERFACE partie 100).
  ffmpeg allégé compilé dans `Logiciel\ffmpeg\allege` (8.1.3, 9 Mo, LGPL, sans OpenH264) et posé à côté de `feanor.exe` ; l'export pour le
  montage explique une taille refusée (80738d2). 3751 tests réussis en deux passages. Version portable FEANOR-portable-2026-11-05c.
  Reste : mention de licence dans À propos, double clic et bouton à essayer à la vraie souris.
- Contenu de la V1 écrit à partir des mots d'Anthony (`suivi/V1.md`), avec les écarts relevés dans le code. À corriger par lui ;
  il signale encore de la friction entre vignettes et storyboard, à détailler.
- ffmpeg 8.1 LGPL installé sur le PC d'Anthony, à sa demande : `Logiciel\ffmpeg\` et copie à côté de `target\release\feanor.exe`
  (FEANOR utilisait jusqu'ici celui de Krita). Essais d'encodage H.264, ProRes, DNxHR et GIF réussis ; 3745 tests réussis avec lui.
  Pas encore distribuable : 126 Mo, et OpenH264 inclus. Il faudra une version allégée sans OpenH264, ses sources et la mention de licence.
- Le logiciel s'appelle FEANOR : fusion fbd6a98 (nom, couleurs Atelier et Papier, Joyau, animation d'ouverture une fois,
  polices Big Shoulders et IBM Plex, barres irisées, rouge réservé aux erreurs bloquantes, point ambre pour les avertissements).
  L'exécutable devient `target\release\feanor.exe`. L'extension `.shot` et les dossiers `SHOT` restent.
- Mémoire d'une longue séance : fusion 9c3e516. 300 plans après 2000 gestes : de 6,5 à 5,8 Go (mesure de banc).
  Les dessins que plus rien n'utilise quittent la mémoire ; une relecture n'a trouvé aucun scénario de perte.
- Essais : 3743 réussis, 0 échec, deux passages sur la branche dont la fusion est identique. Projet d'essai rouvert avec feanor.exe.
- En attente de toi : tests à l'écran (INTERFACE parties 96, 97 et 98).
- Pas fait : focus clavier jade sur les boutons. Inexpliqué : 2,6 à 3 Go hors du tas, sans doute le pilote graphique.
- Sortes de calques en couleurs FEANOR, vectoriel en jade sombre (INTERFACE partie 98). Nuancier « FEANOR », texte copié « FEANOR : », ligne « Anciennement » retirée.
- Dossiers de travail `feanor` et `memoire-seance` et leurs branches supprimés, avec ton accord.
- Version portable FEANOR-portable-2026-11-05 (52a25c9), sans ffmpeg ; zip rouvert et lancé ici sur une copie du projet d'essai.
- Gris sombre redevient le thème de base, avec un trait jade sous l'espace actif, une pastille jade sur l'onglet non enregistré et les couleurs FEANOR des calques (INTERFACE partie 99). 3745 essais réussis, un passage complet. Portable FEANOR-portable-2026-11-05b.

## 2026-10-30

- Créé le dossier suivi/ : ETAT.md (état réel du projet), NOTES.md (ta boîte de notes), JOURNAL.md (ce fichier).
- Lancé cargo check et cargo test sur main (b201dc1) : résultats dans ETAT.md, section 1.
- Aucun fichier source modifié, aucun commit.
- Décision d'Anthony : SHOT abandonne tout lien avec Storyboard Pro et Toon Boom (import, export, import de
  pinceaux, raccourcis, références), pour pouvoir le commercialiser. Chantier lancé sur la branche `sans-sbp`.
- Engrenage de l'accueil gardé (choix d'Anthony). Dossier de travail `demarrage` supprimé ; `importsbp2` le sera
  après le chantier `sans-sbp`, qui l'utilise.
- Fusionné 17dda2a : SHOT sans Storyboard Pro ni Toon Boom. 3288 essais passent, shot.exe reconstruit
  (40,08 → 39,21 Mo, ~7 600 lignes de code en moins). Projet d'essai rouvert sans problème. Dossier `importsbp2` supprimé.
- Lancé : panneau « Scénario » (le scénario importé est gardé dans le projet et lisible, lié aux plans),
  branche `panneau-scenario`.
- Fusionné 32dd5f5 : panneau Scénario (3308 essais passent, shot.exe reconstruit, projet d'essai rouvert).
- Fusionné 5335c60 : ajustements des vignettes (3326 essais passent, shot.exe reconstruit, projet d'essai rouvert).
  Lancé : vignettes par séquences (branche `vignettes-sequences`).
- `vignettes-ajustements` rendu ; renvoyé pour 3 défauts vus sur capture (cadrage d'ouverture, zoom affiché
  deux fois, réglage « lignes d'un projet neuf » sans effet). Rotation du canevas des vignettes non faite : question à Anthony.
- Avant : `vignettes-ajustements` (continuité, planche dessinable, numéros, lignes de texte, réglages,
  passe-partout des cases, barre du bas). Suivant : vignettes par séquences.
- Décisions d'Anthony : pas de rotation du canevas des vignettes ; dossier de travail `scenario` supprimé ;
  recherche (Ctrl+F) dans le panneau Scénario lancée (branche `scenario-recherche`, dossier `Chantiers\recherche`).
- Fusionné 7e3f7e8 : vignettes par séquences (titres, choix et ordre des séquences affichées, export qui suit
  l'affichage ; 3350 essais passent sur la branche).
- Fusionné 4d8677d : recherche Ctrl+F dans le panneau Scénario (3360 essais passent, shot.exe reconstruit, projet
  d'essai rouvert). Ctrl+F reste « Aller à un plan » hors du panneau Scénario.
- Dossier de travail `recherche` supprimé (accord d'Anthony).
- Lancé : hors-cadres texturé + passe-partout à l'export (branche `vignettes-texture-export`).
- Fusionné e546fc7 : hors-cadres texturé + passe-partout à l'export avec sa propre case (3372 essais passent,
  shot.exe reconstruit, projet d'essai rouvert). À vérifier : l'export de la planche ne montre, pour une case à
  mouvement de caméra, que le cadre A (le dessin hors de A n'y est pas) ; préexistant, pas dû à ce chantier.
- Fusionné 3774514 : outil Dégradé plus complet (format du projet 43 → 44 ; 3400 essais passent). Renvoyé une fois
  avant fusion : un changement de calque en pleine retouche pouvait perdre le dégradé (corrigé, essais ajoutés).
- Fusionné 1df74a1 : planche de références (3425 essais passent, shot.exe reconstruit, projet d'essai rouvert).
- Décisions d'Anthony : post-it écrits directement sur la planche (chantier lancé, branche `postit-en-place`, dossier
  `Chantiers\references` réutilisé) ; cadre autour des éléments choisis : OK ; dossier `degrade` supprimé.
- Lancé : planche de références (branche `planche-references`, dossier `Chantiers\references`).
- Lancé : outil Dégradé plus complet (branche `degrade-avance`, dossier `Chantiers\degrade`).
- Point global (relevé en lecture seule) : solide = tests (3425), enregistrement sûr, compatibilité des projets,
  exports en fond, aucun plantage depuis le 22 oct. À améliorer, dans l'ordre : sept chantiers jamais vus à l'écran
  par Anthony ; ffmpeg non livré ; exports montage jamais ouverts dans un logiciel de montage ; limites d'export
  (cadre A seul, « ? » du PDF, son NTSC) ; menu Exporter chargé ; vocabulaire (Fiche / Propriétés du plan, Arrière-plan).
- Décisions d'Anthony : exports — les 4 familles (vidéo pro, images par plan + PDF, son par piste, montage +
  compositing), menu Exporter regroupé par usage (chantier `exports-v1` lancé) ; ffmpeg livré avec SHOT après
  vérification de la licence ; `memoire-dessins` abandonnée (étiquette `archive/memoire-dessins`).
- Lancé : panneau Bibliothèque (`panneau-bibliotheque`), fusion et menu des calques (`calques-menu`). Fait : logo
  retiré de la barre de menus (`sans-logo-barre`, 9c1d1d1, à fusionner).
- Fusionné 012c17a (post-it : écrire directement sur la planche) et 1c5a102 (logo retiré de la barre de menus) :
  3439 essais passent, shot.exe reconstruit, projet d'essai rouvert (Fichier est bien le premier menu).
- Fusionné bab0087 : numérotation verrouillée pour les projets neufs (décision d'Anthony, case dans Préférences), montage redimensionnable (poignée visible, défilement, hauteur des pistes son). 3564 essais passent, shot.exe reconstruit, projet d'essai rouvert. Public visé (Anthony) : storyboarders d'animation indépendants ou en studio, dans un pipeline de production (exports multiples, ponts vers Kitsu / ShotGrid).
- Fusionné abadcb0 : stabilité et retouches de l'audit (annulation de l'export des calques sans fichier tardif ; marquer un envoi ne renomme plus un .pelure ; bouton « Transformer en calque d'animation » ; Échap ferme Exporter ; fin d'export qui s'efface seule ; crayonnes.json « Mettre de côté »). 3550 essais passent (3 passages de suite chez l'agent, 1 chez moi), shot.exe reconstruit, projet d'essai rouvert.
- Fusionnés : exports V1 (c1594e9 : menu en 4 familles, vidéo pro, images par plan, PDF au choix avec accents, son par piste, FCPXML, calques séparés, caméra) et calques (1e6ff18 : fusion fidèle, en tâche de fond si longue, menu clic droit unique). shot.exe reconstruit, projet d'essai rouvert. Suite complète : 3539 à 3541 essais verts sur 3542 selon les passages ; deux essais instables sous charge (annulation de l'export des calques, enregistrement en tâche de fond) confiés au chantier `stabilite-essais` avec les petits bugs de l'audit pratique (envoi qui renomme, calque vidéo, Échap, messages d'export, crayonnes.json).
- Audit pratique à la souris : résumé ajouté à suivi/AUDIT-2026-10-30.md.
- Fusionné : panneau Bibliothèque (3476 essais passent, shot.exe reconstruit, projet d'essai rouvert sans plantage).
- Audit extérieur (storyboard série / long) rangé dans suivi/AUDIT-2026-10-30.md : 3/10 série, 4/10 long, 6/10
  indépendant ; priorités : valider les exports montage, PDF de studio, endurance sur un vrai épisode, équipe, modèle d'épisode.
- À retenir pour après la V1 (demande d'Anthony) : retravailler la partie collaborative (travail en équipe).
- Fusionnés (6040504) : fil de décision par plan (statut des notes, fil du plan, conflits, compteurs, note suivante,
  pastille, relecture n+1 avec réponses du relecteur, retakes + CSV) et premier contact (visite « Premier jour »
  en 5 étapes, Labo caché derrière « Afficher les fonctions expérimentales », Propager rangé dans Animation,
  vocabulaire : Fiche du plan, Fond de la feuille, Vectoriel, tutoiement). 3596 essais passent, shot.exe reconstruit,
  projet d'essai rouvert sans plantage. Format du projet inchangé (44).
- Fusionné 3cd07b7 : pipeline étape 1 (modèle de noms, série / épisode, liste des plans CSV/JSON profil Kitsu,
  paquet de livraison avec manifeste et empreintes, modèle d'épisode). Format du projet 44 → 45. 3627 essais passent,
  shot.exe reconstruit, projet d'essai rouvert. En-têtes Kitsu à confirmer sur une vraie instance.
- Lancés : petits livrables (`petits-livrables`), journal des différences entre envois (`journal-differences`),
  noms de marques retirés + « Papier » (`noms-papier`).
- Fusionné c442f0e + 46d46c4 : plus aucun nom de marque dans l'interface (noms de formats gardés), « Papier » partout
  (anciens noms trouvés par Ctrl+K). 3633 essais passent, shot.exe reconstruit, projet d'essai rouvert.
- Lancés (choix d'Anthony) : pont Kitsu étape 2 (`pont-kitsu`, faux serveur local) et test d'endurance (`endurance`).

## 2026-10-31

- Fusionnés (6fd6808, 634ffec, c0e8bba, 1353610, 7dfac45) : petits livrables (budget de durée, rapport de
  numérotation, planche PDF légendée, PDF de studio, fenêtres dans le dossier du projet, bobine EDL `S01P012A`),
  journal des différences entre envois (PDF + page HTML), pont Kitsu (publier, lire les retours ; adresses à confirmer
  sur une vraie instance), endurance (épisode de 200 plans mesuré, docs/ENDURANCE.md ; enregistrement et copie de
  secours sous la priorité de l'interface). Format du projet 45 → 46. 3701 essais passent, shot.exe reconstruit,
  projet d'essai rouvert sans plantage.
- Endurance, restes : mémoire 6,8 Go à l'ouverture d'un épisode de 200 plans (9–10 Go après une séance) ; dessins des
  plans supprimés gardés jusqu'à la réouverture ; envoi 90–110 ms sur le fil de l'interface ; MP4 lourd (2,7 Go / 15 min).
- Lancé (accord d'Anthony) : chantier mémoire des longs épisodes (branche `memoire-episode`, dossier
  `Chantiers\exports` réutilisé) ; but : 300 plans sous ~4 Go, sans perte ni latence.
- Fusionné 5f8c27a : mémoire des longs épisodes (dessins non regardés rangés en mémoire sans perte, aussi
  l'historique, les caches de traits, les plans supprimés, les onglets cachés ; marquer un envoi par morceaux).
  200 plans : 6,9 → 2,0 Go à l'ouverture, 10,1 → 5,7 Go après 2 000 gestes ; 300 plans : 9,0 → 2,3 Go à l'ouverture.
  Relecture ciblée (second agent) : aucune perte de dessin ; 4 remarques corrigées avant fusion (38528d9). Format
  inchangé (46). 3719 essais passent, shot.exe reconstruit, projet d'essai rouvert (même écran qu'avant).
  Essai intermittent `fermeture_tests::a_rich_project_opened_and_left_idle…` : panique audio (cpal, COM) quand la
  machine manque de mémoire, pas un défaut des dessins.
- À décider avec l'œil d'Anthony : affichage Direct3D 12 au lieu de Vulkan (−2,6 Go, lecture 3× plus rapide selon
  l'agent ; `WGPU_BACKEND=dx12`), jamais vérifié à l'écran.
- Dossiers de travail vignettes, bibliotheque, calques, references, exports supprimés et leurs 6 branches fusionnées
  effacées (accord d'Anthony).

## 2026-11-01

- Storyboard Pro fermé sans enregistrer (accord d'Anthony) : il occupait 14 Go, et Windows refusait d'ouvrir de nouvelles
  fenêtres SHOT.
- Affichage Direct3D 12 : il n'est pas compilé dans SHOT. `WGPU_BACKEND=dx12` fait passer à OpenGL, donc les chiffres
  « Direct3D 12 » du chantier mémoire n'étaient pas ceux de Direct3D 12. Docs corrigées. Les raccourcis de comparaison ont été
  retirés. Activer Direct3D 12 demanderait de télécharger deux crates.
- Téléchargé, avec l'accord d'Anthony : gpu-allocator, range-alloc et presser, depuis crates.io. Direct3D 12 est compilé
  (branche `affichage-dx12`).
- Vraie mesure sur l'épisode de 200 plans, après 120 changements de plan : 3,6 Go avec Direct3D 12, 3,9 Go avec Vulkan. Direct3D 12
  est un peu moins fluide. Vulkan reste par défaut ; Direct3D 12 sert de repli avant OpenGL.
- Fusionné 0bc89a7 : 3719 essais passent, shot.exe reconstruit, projet d'essai rouvert (même écran, Vulkan).
- Avec l'accord d'Anthony : dossier Chantiers\dx12 et branche affichage-dx12 supprimés.

## 2026-10-29

- Fusionné dans main : import Storyboard Pro (dessins, caméras, son, puis rendu et vitesse), clés d'opacité et calque
  « Couleur unie », démarrage sans projet ouvert, fenêtre d'accueil compacte.
- Projet Storyboard Pro lourd : la mémoire d'un animatic importé passe de 20,7 à 10,8 Go, l'import de 181 s à 17 s
  (mesures de l'agent, banc sans fenêtre). À confirmer par ton test sur ton fichier de 7 plans.
- Version portable SHOT-portable-2026-10-29b.
- En attente de toi : garder ou retirer l'engrenage de l'accueil ; suppression des dossiers de travail `demarrage`
  et `importsbp2`.
