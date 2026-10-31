# Journal de Claude

Ce que Claude a fait ou proposé, par session, le plus récent en haut. Une session = une date.
Pour le détail technique, voir l'historique git et docs/INTERFACE.md (une partie numérotée par chantier).

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

## 2026-10-29

- Fusionné dans main : import Storyboard Pro (dessins, caméras, son, puis rendu et vitesse), clés d'opacité et calque
  « Couleur unie », démarrage sans projet ouvert, fenêtre d'accueil compacte.
- Projet Storyboard Pro lourd : la mémoire d'un animatic importé passe de 20,7 à 10,8 Go, l'import de 181 s à 17 s
  (mesures de l'agent, banc sans fenêtre). À confirmer par ton test sur ton fichier de 7 plans.
- Version portable SHOT-portable-2026-10-29b.
- En attente de toi : garder ou retirer l'engrenage de l'accueil ; suppression des dossiers de travail `demarrage`
  et `importsbp2`.
