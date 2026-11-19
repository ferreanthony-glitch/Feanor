# Banque 3D de FEANOR : d'où viennent les modèles

Les 45 modèles de ce dossier ont été choisis par Anthony le 19 novembre 2026 parmi cinq kits de Kenney (www.kenney.nl),
téléchargés depuis OpenGameArt le même jour. Chaque archive contenait un `License.txt` qui dit :

> License: (Creative Commons Zero, CC0) — http://creativecommons.org/publicdomain/zero/1.0/
> You can use this content for personal, educational, and commercial purposes.
> Support by crediting 'Kenney' or 'www.kenney.nl' (this is not a requirement)

La licence CC0 place ces modèles dans le domaine public : on peut les vendre avec FEANOR, les modifier, sans rien demander.
Créditer Kenney n'est pas obligatoire ; FEANOR le fait tout de même, ici et dans « À propos ».

| Famille | Kit | Source | Modèles gardés |
|---|---|---|---|
| Nature | Nature Kit 2.1 | https://opengameart.org/content/nature-kit | 21 |
| Banlieue | City Kit (Suburban) 2.0 | https://opengameart.org/content/city-kit-suburban | 6 |
| Routes | City Kit (Roads) 2.2 | https://opengameart.org/content/city-kit-roads | 8 |
| Commerces | City Kit (Commercial) 2.1 | https://opengameart.org/content/city-kit-commercial | 3 |
| Ville rétro | Retro Urban Kit 2.0 | https://opengameart.org/content/retro-urban-kit | 7 |

## Ce qui a été changé

- Les 24 modèles des kits de ville renvoyaient à une texture voisine (`Textures/colormap.png`, environ 11 Ko). Elle est maintenant
  rangée dans chaque fichier GLB, qui se suffit à lui-même. La géométrie, les matières et les couleurs ne changent pas.
- Rien d'autre : pas de simplification, pas de retouche.

`catalogue.json` donne pour chaque modèle sa famille, son fichier, son aperçu, son nombre de triangles, sa taille, son kit,
son auteur, sa licence et sa source. Les aperçus (`apercus/`) sont ceux fournis dans les kits.
