# Presets Storyboard Pro (référence)

Fichiers fournis par Anthony (archive `STB_SETUP.7z`, avril 2023) :

| Fichier | Contenu | Usage dans Pelure |
|---|---|---|
| `STB_Shortcuts.xml` | 8 raccourcis **personnalisés** (seulement les différences avec les raccourcis d'usine) | repris tels quels dans `keymap.toml`, source « perso » |
| `STB_Preferences.xml` | préférences de Storyboard Pro (pelure, table lumineuse, zones de sécurité, sauvegarde auto…) | valeurs par défaut de référence pour les phases à venir |

Non copiés ici, mais présents dans l'archive : `STB_TextureBrushes_preset.xml` (9 pinceaux, dont 6
texturés avec 24 textures intégrées ; à étudier en phase 3), une texture de papier, et deux scripts
tiers (réordonner / renommer des calques en lot), qui ne sont pas exécutés.

## Raccourcis d'usine

Les raccourcis par défaut **ne sont pas** dans l'archive. Ils ont été lus (sans copier le fichier,
qui appartient à Toon Boom) dans l'installation locale :

`C:\Program Files (x86)\Toon Boom Animation\Toon Boom Storyboard Pro 27\resources\sb_shortcuts.xml`

- 473 actions, dont **65 avec une touche par défaut**. Toutes sont dans `keymap.toml`, avec leur
  identifiant d'origine dans le champ `source`. Un test vérifie qu'aucune n'est oubliée ni inventée.
- Format : `<shortcut id="…" value="Ctrl+Z" text="…" condition="…"/>` rangés par `<category>`.
  Les fichiers personnels ajoutent un enfant `<flavour key="…" id="Toon Boom Storyboard"/>`.
- Certaines entrées portent une `condition` (ex. `not express`, `hasPaintUnpaintedTool`). On a
  supposé qu'elles s'appliquent à Storyboard Pro 27 complet.
- **Module « Guides »** (relevé en 1c) : `Plugins\GuideTool\resources\sb_shortcuts.xml` ajoute 4
  touches pour la saveur « Toon Boom Storyboard » : G (activer le guide), Maj+A (aligner sur le
  guide), Maj+L (verrouiller), Maj+G (lignes de référence). Elles sont dans `keymap.toml`. Maj+A,
  jusque-là proposé pour « scène précédente », a donc été rendu au guide.
- Certaines entrées ont des valeurs différentes selon la saveur (`<valueList flavour="…">`). On prend
  toujours « Toon Boom Storyboard » (ex. table lumineuse : aucune touche, alors que Harmony a Maj+L).

Le même dossier contient `shortcuts.xml` (629 entrées : vue nodale, feuille d'exposition…), qui
semble être celui de Harmony. **Il n'a pas été utilisé** : impossible de savoir avec certitude ce que
Storyboard Pro en charge. À confirmer si un raccourci manque (exemples d'entrées qui pourraient
concerner le dessin : outil perspective `Alt+0`, fermer les trous `Alt+C`, lisser `Alt+Maj+S`).

## Interprétation pour le clavier AZERTY

Les raccourcis de Storyboard Pro sont écrits avec des caractères (« Q », « , », « [ »). Choix retenus,
à corriger si besoin :

- **Lettres : le caractère tapé.** Tes réglages Q / W / X / Maj+S / Maj+D tombent tous sous la main
  gauche en AZERTY : ils ont été choisis d'après les lettres de ton clavier.
- **Chiffres et ponctuation : la position de la touche** (nommée comme sur un QWERTY). Sinon « 1 »,
  « . » ou « [ » demanderaient Maj ou AltGr en AZERTY. En AZERTY : `,` → touche `;`, `.` → touche `:`,
  `[` → `^`, `]` → `$`, `-` → `)`, `1` → `&/1`.
- On ne sait pas exactement comment Storyboard Pro lui-même traite ces cas sur un clavier AZERTY.
