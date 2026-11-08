# Journal de Claude

Ce que Claude a fait ou proposé, par session, le plus récent en haut. Une session = une date.
Pour le détail technique, voir l'historique git et docs/INTERFACE.md (une partie numérotée par chantier).

## 2026-11-08

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
