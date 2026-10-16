# Propagation de la peinture (façon EbSynth)

On peint quelques images clés sur un calque bitmap. « Propager » fabrique les autres images de ce
calque en suivant le mouvement d'un guide :
- une vidéo ;
- ce qu'on voit sous le calque ;
- un calque choisi, par exemple la ligne d'une animation faite dans Pelure.

Le moteur est la caisse [`synthese/`](../synthese/README.md) : synthèse par l'exemple, flux
optique, mélange des clés, carte graphique. L'intégration dans Pelure est dans `src/propager.rs`
(sans interface, testé) et `src/propagerui.rs` (fenêtre, tâche, progression).

## S'en servir

1. Mettre sous le calque à peindre ce que la peinture doit suivre. Ce peut être une vidéo (Fichier
   > Importer une vidéo), ou une animation dessinée dans Pelure, en vectoriel ou en bitmap.
2. Sur un calque bitmap, peindre une ou plusieurs images clés, en laissant transparent ce qui doit
   rester visible dessous.
3. Clic droit sur la ligne du calque peint > « Propager la peinture… ». La commande
   `calque.propager` peut aussi recevoir un raccourci (clic droit > Assigner un raccourci).
4. Dans la fenêtre, choisir **ce que la peinture suit** :
   - **la vidéo** d'un calque vidéo ;
   - **ce qu'on voit sous le calque** : tous les calques visibles dessous, composés comme à
     l'affichage ;
   - **un calque seul**, même caché : par exemple la ligne de l'animation, sans le décor.

   Puis **les images à refaire** : « De l'image … à l'image … » (numérotées comme la timeline :
   image, time code ou trame ; on glisse ou on tape). Par défaut, toute la plage du guide.
   « Tout » y revient ; « Entrée → sortie » reprend les points d'entrée et de sortie de la timeline.
   Les images hors de la plage ne changent pas. Le choix est gardé pour ce calque et ce guide.

   La fenêtre dit aussi ce qui va se passer : les images, les clés (et celles hors plage, ignorées),
   le nombre d'images à fabriquer. On y choisit le calcul (carte graphique ou processeur) et les
   réglages.
5. « Propager » : une fenêtre montre la progression, le temps restant et la dernière image
   fabriquée. « Arrêter » abandonne sans rien changer.
6. À la fin, les images sont posées en une seule étape d'annulation (Ctrl+Z).

## Choix d'Anthony (16 octobre 2026)

- **Les images fabriquées vont dans le calque peint lui-même.** Ses clés y sont marquées « Clé »
  (rôles de TVPaint 12). Pour corriger, on retouche une clé (ou on en marque une nouvelle) puis on
  repropage : seules les images marquées « Clé » sont gardées.
- **La clé est le calque peint seul, avec sa transparence.** Ce qui est dessous reste visible là où
  rien n'est peint, et la transparence est propagée elle aussi.
- **Les guides sont tous proposés** : la vidéo, ce qu'on voit sous le calque, ou un calque choisi.
  Le premier proposé est le calque vidéo sous le calque peint s'il y en a un, sinon ce qu'on voit
  dessous.

## Comment

- **Les clés.** Ce sont les images marquées « Clé » qui ont un dessin, s'il y en a. Sinon, chaque
  image où un dessin commence (une exposition de plusieurs images compte une fois). Elles sont
  cherchées sur toute la plage du guide : une exposition commencée avant la plage choisie n'est
  pas une clé.
- **La plage du guide** (`propager::span`).
  - Avec une vidéo : les images du plan où elle a une image.
  - Avec des calques : du début du plan jusqu'à la dernière image dessinée de ces calques (ou la fin
    de leur vidéo). Au-delà, le guide ne bouge plus.
- **La plage refaite.** Toute celle du guide, ou celle choisie à la main, ramenée dans celle du
  guide. Il faut au moins une clé dedans, sinon la fenêtre refuse et dit où sont les clés du
  calque. Les clés hors plage ne servent pas, mais elles sont marquées « Clé » elles aussi : sans
  ça, après une propagation partielle, la suivante ne verrait plus que les clés marquées et les
  perdrait.

  On recule avant la première clé de la plage, on avance après la dernière, et on mélange entre
  deux clés.
- **L'espace de travail.** On travaille en RVBA prémultiplié : les moyennes des patchs restent
  justes au bord de la peinture.
  - Avec une vidéo : le rectangle où `video::read_frame` pose ses images (au centre du cadre).
  - Avec des calques : le cadre.

  La peinture hors de ce rectangle n'est pas propagée.
- **Les images du guide.** Celles d'une vidéo sont lues dans ses fichiers. Celles d'un guide fait de
  calques sont composées sur le papier, comme l'affichage les compose (transformation et défilement
  multiplan, effets, CTG, vidéo), mais seulement pour les calques du guide (`propager::render_guide`).
  La tâche les demande au fil de l'interface, seul à toucher au document, et le réveille pour qu'il
  réponde tout de suite.
- **Les poids des guides.** Ce sont ceux d'EbSynth : image du guide 4, position suivie par le flux
  optique 10, image précédente 1, clé 1. On peut les régler dans la fenêtre.
- **La tâche.** Elle relève tout sur le document au départ (`propager::plan`), puis tourne sur un
  autre fil. Les images fabriquées arrivent au fur et à mesure. Pendant ce temps, le reste de
  l'interface attend (`modal_open`).
- **À la fin.** Le plan et le calque sont retrouvés par leur identifiant. On prend un instantané,
  on crée un dessin neuf par image (`propager::apply`), on marque les clés, puis
  `commit_structure` en fait une seule étape d'annulation.

## La carte graphique

La synthèse tourne sur le périphérique wgpu de l'affichage, celui d'eframe, partagé avec
l'affichage : il est pris dans `cc.wgpu_render_state` quand l'appli démarre. Ce périphérique garde
les limites par défaut de wgpu (128 Mo par tampon) : une image HD y tient, une image 4K non.
`Gpu::fits` le vérifie avant d'envoyer quoi que ce soit, car une erreur de validation arrêterait le
programme. Sinon, on ouvre un périphérique avec toutes les limites de la carte, et à défaut on
calcule sur le processeur. Avec l'affichage en OpenGL, on calcule sur le processeur.

## Mesures

Sur l'animatique BRAWLIDAYS en 960×540, 75 images avec une clé partielle à l'image 30 (l'enseigne
seulement), sur la RTX 2080 SUPER : 74 images fabriquées en 20 s dans l'appli. C'est le même temps
avec le guide vidéo et avec « ce qu'on voit dessous », où les images sont composées par
l'interface. Sur le processeur, compter environ 4,6 s par image en 960×540, et 18 s en 1920×1080.

## Essais

- `src/propager_tests.rs` couvre :
  - la plage, les clés (dessins ou rôles « Clé »), les guides proposés et les refus ;
  - la plage choisie à la main : seules ses images sont refaites, les clés hors plage sont
    ignorées (mais marquées), refus sans clé dans la plage ;
  - le rectangle de travail et l'aller-retour entre dessin et image ;
  - la composition des guides (calques dessous ; un calque seul, même caché) ;
  - de vraies propagations : sur une petite vidéo, et sans vidéo sur une animation faite de dessins
    (la peinture suit le décor qui glisse de 2 pixels par image) ;
  - l'arrêt.
- `demo_project` (ignoré) fabrique un projet de démonstration à ouvrir dans l'appli :

```
PELURE_DEMO_IMAGES=<dossier 000001.jpg…> PELURE_DEMO_IMAGE=30 PELURE_DEMO_CLE=<peinture.png>
PELURE_DEMO_RECT=20,120,330,380 PELURE_DEMO_PROJET=<dossier>\propagation.pelure
cargo test demo_project -- --ignored
```

## Limites

- Dans la plage refaite, toutes les images sont remplacées, sauf les clés : pour garder une image,
  la marquer « Clé » ou la laisser hors de la plage. La sélection d'images de la timeline n'est pas
  encore reprise comme plage (seulement l'entrée et la sortie).
- Au bord de la plage, l'image refaite ne tient pas compte de sa voisine hors plage : un saut peut
  se voir au raccord si on ne refait qu'une partie d'une propagation.
- La peinture hors du rectangle de travail (dans la marge de la feuille) n'est pas propagée.
- Le calque peint est pris tel quel sur la feuille : sa propre transformation n'est pas comptée.
- Calques bitmap seulement pour la peinture. Le guide, lui, peut être vectoriel.
- PatchMatch est breveté par Adobe (brevet américain 8,861,869) : à faire vérifier avant de
  distribuer ou de vendre Pelure.
