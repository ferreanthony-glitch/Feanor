# La caméra se souvient (Vignettes, approche A)

Anthony, 2026-11-09 : « repenser Vignettes, quitte à repartir de zéro […] pour l'instant il n'agit pas encore comme une grande planche de
dessin sur laquelle des caméras sont posées, et peuvent avoir une action sur le dessin sous certaines conditions dont nous avons parlé. »
Cinq approches présentées (A à E) ; son choix : « Ok on essaye A ». Branche `camera-souvient`, partie 131 de `docs/INTERFACE.md`.

## La règle, en une phrase

**Un trait appartient tout entier à la case où le stylet s'est posé ; hors de toute case, il appartient au papier. Le bord d'une case est
une étiquette, jamais des ciseaux.**

## Ce que ça change

| Avant (parties 111 à 128) | Maintenant |
|---|---|
| Au lâcher, chaque point du trait cherche sa case : le trait est coupé en parts (case d'origine, cases voisines, papier), reliées par un numéro (`lien`). | Une seule question au lâcher : où le trait a-t-il commencé ? Le trait reste entier, à un seul endroit. |
| Une case montre son dessin jusqu'à 12 % au-delà de son cadre ; le reste était passé au papier ou aux voisines. | Une case montre **tout** son dessin sur la planche, où qu'il aille (la vignette s'étend à l'encre du plan). Le film et les exports du plan restent coupés au cadre. |
| Le bout d'un trait qui tombe dans la case pâle suivante la fait naître (partie 113, décision 8). | Une case pâle naît quand on pose le stylet dedans **ou dans sa marge**. Le bout d'un trait ne fait plus rien naître. |
| Une case qui naît prend la **part** des traits du papier dans son champ (partie 128). | Elle prend ces traits **entiers**. |
| La sélection (lasso Ctrl) rassemble les parts, puis les répartit de nouveau. | Elle rassemble, puis pose le trait entier là où il commence. Un vieux trait coupé redevient entier dès qu'on le déplace. |

## Les décisions de détail

1. **Le propriétaire** est celui du **premier point** du trait : l'image d'une case d'abord (l'intérieur d'un cadre a toujours priorité sur
   la marge d'une autre, partie 111), puis sa marge (le débord des Préférences, 12 % d'origine), puis une case pâle (son cadre ou sa marge :
   son plan naît), sinon le papier.
2. **Le papier** reste le calque « Hors cadres » de la planche (vectoriel avec la texture du pinceau, ou pixels sur la couche bitmap de la
   planche) : attaché à la case la plus proche, dans aucun plan. Où il se peint : voir la décision 3.
3. **La feuille ou le film.** Sur la planche, **rien n'est caché par une case** : le papier et ce que chaque case a dessiné hors de son
   cadre se peignent **par-dessus les images de toutes les cases** (sous leurs filets, leurs numéros et leurs notes). Un trait parti de la
   case 1 qui traverse la case 2 se voit donc en entier, dans la case 2 aussi, bien qu'il ne soit pas dans son plan. **Le passe-partout
   montre le film** : chaque case ne montre plus que son plan, tout le reste passe dessous et le dehors est caché. (Premier jet : le débord
   passait sous les autres cases ; changé avant tout essai, parce que la règle acceptée dit « ce qui dépasse du cadre reste visible sur la
   planche » et que ses retours ont toujours visé des traits coupés ou cachés.) L'export de la planche suit la même règle.
4. **Les deux conditions d'Anthony** (une caméra qu'on déplace emporte son dessin ; un mouvement de caméra fait de la place) : le dessin est
   rangé dans le plan, donc il suit sa case sans rien faire de plus ; le papier suit la case à laquelle il est attaché.
5. **Bitmap** : les pixels d'un trait posé dans une case restent dans son dessin, au-delà du cadre aussi (les tuiles hors de la feuille,
   déjà enregistrées) ; posé hors de toute case, le trait va entier sur la couche bitmap de la planche.
6. **Une case qui ne peut pas recevoir** (calque verrouillé, masqué… partie 122) : on n'y dessine pas, comme dans la vue Dessin. Un trait
   qu'un déplacement au lasso y mène reste sur le papier, par-dessus elle, entier.
7. **Anciens projets** : ouverts tels quels, sans conversion ; le format du projet ne change pas. Les parts d'un même numéro restent des
   parts, que le lasso prend toujours ensemble.
8. **La borne** : la vignette d'une case s'étend jusqu'à quatre cadres de chaque côté (et 4096 pixels en tout) ; au-delà, le trait est
   dans le dessin mais ne se voit plus sur la planche. Une caméra basculée ou en panoramique (perspective) montre aussi ce qui dépasse,
   mais le trait s'amincit avec la perspective de sa feuille : il n'est pas continu sur la planche (le film et les exports sont justes).
9. **La marge d'une case pâle** : seule la **première** case pâle d'une séquence naît d'un appui dans sa marge, et pas à sa gauche, qui
   reste la zone de la barre de continuité. Plus loin, la marge d'une case pâle est du papier.
10. **La gomme** (ajoutée en cours de chantier, `gommedebord.rs`) : ce qu'on voit sur le papier se gomme sur le papier. La gomme partie
    hors de toute image efface aussi ce que les cases ont dessiné hors de leur cadre (un trait vectoriel : entier, si elle le touche hors
    du cadre ; des pixels : ceux sous son cercle) ; dans l'image d'une case, le dessin de cette case reste à elle. Une seule étape
    d'annulation pour le geste. Rien quand le storyboard est verrouillé, rien avec le passe-partout (on ne gomme pas ce qu'on ne voit pas).
11. **Les dessins que plusieurs cases montrent gardent la règle d'avant** (décidé après la relecture indépendante) : le **Décor** (un
    seul dessin pour toutes les cases d'un plan), un calque partagé (**planche-mère**), un calque de toute la scène, un dessin tenu d'une
    marque d'image à l'autre (`render::dessins_communs`). Entier, un trait y déborderait autour de **chacune** des cases qui le montrent,
    par-dessus leurs voisines ; et une planche-mère vue par trois caméras s'étalerait trois fois sur la planche. Pour eux : un trait
    tracé sur la planche est réparti case par case comme avant, chaque case en montre son cadre et sa marge ordinaire (12 %), la gomme de
    la planche n'y touche pas, le lasso ne les prend que près de leur case. C'est aussi ce qu'Anthony avait demandé : le Décor reste tel
    quel, et un plan ne montre pas toute une planche en baladant sa caméra.
12. **Un trait bitmap parti du papier** qui traverse le dessin d'une case va sur le papier **seul** : là où il recouvre de l'encre, on
    retrouve ce que le trait a posé (`horschamp::part_du_trait`) au lieu d'emporter le mélange, qui laissait un double du dessin de la
    case sur le papier.

## Limites connues

- Caméra en perspective : voir la décision 8.
- Le trait d'une case qui passe dans l'image d'une autre s'y voit mais ne s'y gomme pas depuis cette autre case (depuis le papier, depuis
  sa case, ou lasso puis Suppr).
- Un plan vectoriel qui naît ne prend pas les morceaux bitmap du papier.
- Un trait du Décor qu'on mène au lasso hors du champ de sa case passe sur la planche (il ne se verrait pas loin de sa case).
- Dessiner vite plusieurs traits qui sortent d'une même case : le bout hors cadre d'un trait d'avant peut manquer un instant, le temps
  que la vignette de la case se refasse (une fraction de seconde ; rien n'est perdu).
- Mémoire : une case très zoomée dont le dessin s'étend loin peut porter une vignette de 4096 pixels de côté (64 Mo) tant qu'elle est à
  l'écran.

## La relecture indépendante (2026-11-09)

Demandée par l'autre session avant toute fusion. Quinze remarques ; corrigées : la gomme qui prenait un trait resté dans son cadre, qui
ignorait le verrou du storyboard et le passe-partout, qui gommait hors de ce que la planche montre, son coût (elle ne lit plus que les
cases dont le dessin arrive sous elle) et son compte d'étapes si on annule pendant le geste ; les dessins communs (décision 11) ; le
double du dessin sur le papier (décision 12) ; la marge ordinaire d'une case qui disparaissait dans un grand export ; ce que la planche
savait du débord d'une case après un rendu d'arrière-plan ; les messages restés sur l'ancienne règle. Notées en limites : les deux
dernières de la liste ci-dessus.

## Mesure

60 cases, 2 040 traits, longs débords : repos en 1,2 s ; 65 vignettes gardées, 23,4 Mo ; une image d'écran en 0,89 ms (médiane), 1,63 ms
(la pire) ; un trait de plus puis le repos : 0,3 s.

## À juger à l'essai (Anthony)

- Un trait parti de la case 1 qui traverse la case 2 **se voit dans la case 2** sur la planche, sans être dans son plan ; le passe-partout
  le retire.
- Un trait commencé entre deux cases ou dans la bande de note reste du papier, même s'il entre ensuite dans une case.
- Le bout d'un trait ne fait plus naître la case pâle suivante.
- Une caméra zoomée montre autour de sa case tout le dessin de sa feuille.
