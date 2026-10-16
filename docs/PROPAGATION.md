# Propagation de la peinture sur une vidéo (façon EbSynth)

On peint quelques images clés sur un calque posé sur un calque vidéo. « Propager » fabrique les
autres images de ce calque, sur toute la durée de la vidéo dans le plan, en suivant le mouvement de
la vidéo. Le moteur est la caisse [`synthese/`](../synthese/README.md) : synthèse par l'exemple,
flux optique, mélange des clés, carte graphique. L'intégration dans Pelure est dans
`src/propager.rs` (sans interface, testé) et `src/propagerui.rs` (fenêtre, tâche, progression).

## S'en servir

1. Importer une vidéo (Fichier > Importer une vidéo) : un calque vidéo.
2. Ajouter un calque bitmap au-dessus et peindre une ou plusieurs images clés, là où on veut, en
   laissant transparent ce qui doit rester la vidéo.
3. Clic droit sur la ligne du calque peint > « Propager la peinture sur la vidéo… ». La commande
   `calque.propager` peut aussi recevoir un raccourci (clic droit > Assigner un raccourci).
4. La fenêtre dit ce qui va se passer : la vidéo, les images, les clés, le nombre d'images à
   fabriquer. On y choisit le calcul (carte graphique ou processeur) et les réglages.
5. « Propager » : une fenêtre montre la progression, le temps restant et la dernière image
   fabriquée. « Arrêter » abandonne sans rien changer.
6. À la fin, les images sont posées en une seule étape d'annulation (Ctrl+Z).

## Choix d'Anthony (16 octobre 2026)

- **Les images fabriquées vont dans le calque peint lui-même.** Ses clés y sont marquées « Clé »
  (rôles de TVPaint 12). Pour corriger, on retouche une clé (ou on en marque une nouvelle) puis on
  repropage : seules les images marquées « Clé » sont gardées.
- **La clé est le calque peint seul, avec sa transparence.** La vidéo reste visible là où rien
  n'est peint, et la transparence est propagée elle aussi.

## Comment

- **Les clés.** Ce sont les images marquées « Clé » qui ont un dessin, s'il y en a. Sinon, chaque
  image où un dessin commence (une exposition de plusieurs images compte une fois).
- **La plage.** Ce sont toutes les images du plan où la vidéo a une image : on recule avant la
  première clé, on avance après la dernière, et on mélange entre deux clés.
- **L'espace de travail.** C'est le rectangle où `video::read_frame` pose les images de la vidéo
  sur la feuille (au centre du cadre), en RVBA prémultiplié : les moyennes des patchs restent justes
  au bord de la peinture. La peinture hors de ce rectangle n'est pas propagée.
- **Les guides.** Ce sont ceux d'EbSynth, avec les mêmes poids par défaut : vidéo 4, position
  suivie par le flux optique 10, image précédente 1, clé 1. On peut les régler dans la fenêtre.
- **La tâche.** Elle relève tout sur le document au départ (`propager::plan`), puis tourne sur un
  autre fil. Les images fabriquées arrivent au fur et à mesure. Pendant ce temps, le reste de
  l'interface attend (`modal_open`).
- **À la fin.** Le plan et le calque sont retrouvés par leur identifiant. On prend un instantané,
  on crée un dessin neuf par image (`propager::apply`), on marque les clés, puis
  `commit_structure` en fait une seule étape d'annulation.

## La carte graphique

La synthèse tourne sur le périphérique wgpu de l'affichage, celui d'eframe, partagé avec
l'affichage : il est pris dans `cc.wgpu_render_state` quand l'appli démarre. Ce périphérique garde
les limites par défaut de wgpu (128 Mo par tampon) : une vidéo HD y tient, une vidéo 4K non.
`Gpu::fits` le vérifie avant d'envoyer quoi que ce soit, car une erreur de validation arrêterait le
programme. Sinon, on ouvre un périphérique avec toutes les limites de la carte, et à défaut on
calcule sur le processeur. Avec l'affichage en OpenGL, on calcule sur le processeur.

## Mesures

Sur l'animatique BRAWLIDAYS en 960×540, 75 images avec une clé partielle à l'image 30 (l'enseigne
seulement) : 74 images fabriquées en 20 s dans l'appli, sur la RTX 2080 SUPER. Sur le processeur,
compter environ 4,6 s par image en 960×540, et 18 s en 1920×1080.

## Essais

- `src/propager_tests.rs` : plage, clés (dessins ou rôles « Clé »), refus, rectangle de la vidéo,
  aller-retour entre dessin et image, vraie propagation sur une petite vidéo (la peinture suit le
  décor qui glisse de 2 pixels par image), arrêt.
- `demo_project` (ignoré) fabrique un projet de démonstration à ouvrir dans l'appli :

```
PELURE_DEMO_IMAGES=<dossier 000001.jpg…> PELURE_DEMO_IMAGE=30 PELURE_DEMO_CLE=<peinture.png>
PELURE_DEMO_RECT=20,120,330,380 PELURE_DEMO_PROJET=<dossier>\propagation.pelure
cargo test demo_project -- --ignored
```

## Limites

- Toute la plage de la vidéo est refaite, sauf les clés. Il n'y a pas encore de plage choisie à la
  main.
- La peinture hors du rectangle de la vidéo (dans la marge de la feuille) n'est pas propagée.
- Calques bitmap seulement : un calque vectoriel ne se propage pas.
- PatchMatch est breveté par Adobe (brevet américain 8,861,869) : à faire vérifier avant de
  distribuer ou de vendre Pelure.
