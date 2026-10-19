//! Raccourcis clavier, lus dans `keymap.toml` et modifiables sans recompiler.
//!
//! Chaque action du logiciel a un identifiant stable (`ACTIONS`). Le fichier associe des touches
//! à ces identifiants. Le fichier par défaut est intégré au programme ; un fichier utilisateur
//! (à côté de l'exécutable, ou dans %APPDATA%\SHOT) remplace action par action ce qu'il redéfinit.

use std::path::PathBuf;

use eframe::egui::{Key, Modifiers};
use serde::{Deserialize, Serialize};

use crate::app::Mode;

/// Fichier de raccourcis par défaut, intégré au programme.
pub const DEFAULT_KEYMAP: &str = include_str!("../keymap.toml");

pub struct ActionInfo {
    pub id: &'static str,
    pub label: &'static str,
    /// La fonction existe dans le logiciel.
    pub implemented: bool,
    /// Le raccourci se répète tant que la touche reste enfoncée.
    pub repeat: bool,
}

const fn act(id: &'static str, label: &'static str, implemented: bool, repeat: bool) -> ActionInfo {
    ActionInfo {
        id,
        label,
        implemented,
        repeat,
    }
}

/// Toutes les actions connues. Les identifiants ne changent jamais : on en ajoute, on n'en renomme pas.
pub const ACTIONS: &[ActionInfo] = &[
    act("fichier.nouveau", "Nouveau projet", true, false),
    act("fichier.ouvrir", "Ouvrir", true, false),
    act("fichier.enregistrer", "Enregistrer", true, false),
    act("fichier.enregistrer_sous", "Enregistrer sous", true, false),
    act("fichier.afficher_explorateur", "Afficher le projet dans l'Explorateur Windows", true, false),
    act("fichier.fermer", "Fermer le projet", true, false),
    act("fichier.quitter", "Quitter", true, false),
    act("fichier.exporter_film", "Exporter le film (vidéo)", true, false),
    act("fichier.exporter_images", "Exporter des images (PNG)", true, false),
    act("fichier.exporter_son", "Exporter le son (WAV)", true, false),
    act("fichier.exporter_pdf", "Exporter le storyboard (PDF)", true, false),
    act("fichier.exporter_image_courante", "Exporter l'image courante (PNG)", true, false),
    act("fichier.exporter", "Exporter (choisir le format : film, images, PDF, son…)", true, false),
    act("fichier.preferences", "Préférences de SHOT (copie de secours, curseur, panneaux, raccourcis)", true, false),
    act("edition.annuler", "Annuler", true, true),
    act("edition.retablir", "Rétablir", true, true),
    act("edition.couper", "Couper", true, false),
    act("edition.copier", "Copier", true, false),
    act("edition.coller", "Coller", true, false),
    act("edition.coller_sur_place", "Coller sur place", true, false),
    act("edition.supprimer", "Supprimer la sélection (rien sans sélection)", true, false),
    act("dessin.vider", "Vider le dessin de l'image courante", true, false),
    act("edition.supprimer_et_resserrer", "Supprimer et resserrer", false, false),
    act("edition.tout_selectionner", "Tout sélectionner", true, false),
    act("edition.selectionner_dessins_scene", "Sélectionner tous les dessins de la scène", false, false),
    act("edition.tout_deselectionner", "Tout désélectionner", true, false),
    act("vue.outil_zoom", "Outil zoom", false, false),
    act("vue.zoom_avant", "Zoom avant", true, true),
    act("vue.zoom_arriere", "Zoom arrière", true, true),
    act("vue.plein_ecran", "Plein écran", true, false),
    act("vue.rotation_horaire", "Tourner la vue (sens horaire)", true, true),
    act("vue.rotation_antihoraire", "Tourner la vue (sens antihoraire)", true, true),
    act("vue.reinit_rotation", "Remettre la vue droite", true, false),
    act("vue.reinit_zoom", "Zoom 100 %", true, false),
    act("vue.ajuster", "Ajuster à la fenêtre", true, false),
    act("vue.pan_libre", "Déplacer la vue (maintenir)", true, false),
    act("vue.rotation_temporaire", "Rotation temporaire (maintenir)", false, false),
    act("vue.zoom_temporaire_avant", "Zoom avant temporaire (maintenir)", false, false),
    act("vue.zoom_temporaire_arriere", "Zoom arrière temporaire (maintenir)", false, false),
    act("vue.grille", "Afficher la grille", false, false),
    act("vue.pelure", "Pelure d'oignon", true, false),
    act("vue.miroir", "Miroir de la vue", true, false),
    act("vue.planches", "Storyboard : vue dessin / vue vignettes", true, false),
    act("mode.basculer", "Passer du storyboard à l'animation, et retour", true, false),
    act("aide.raccourcis", "Liste des raccourcis", true, false),
    act("aide.a_propos", "À propos de SHOT (version, logo)", true, false),
    act("aide.diagnostic", "Diagnostic des performances de SHOT : détecter la machine, mesurer, proposer des réglages", true, false),
    act("aide.palette","Palette de commandes : chercher une commande par son nom", true, false),
    act("outil.pinceau", "Outil pinceau", true, false),
    act("outil.crayon", "Outil crayon", true, false),
    act("outil.gomme", "Outil gomme", true, false),
    act("outil.remplissage", "Outil remplissage", true, false),
    act("outil.peindre_non_peint", "Outil peindre les zones non peintes", false, false),
    act("outil.pipette", "Outil pipette", true, false),
    act("outil.selection", "Outil sélection", true, false),
    act("outil.selection_couleur", "Outil sélection par couleur", true, false),
    act("pinceau.derriere", "Pinceau et crayon : peint derrière (oui / non)", true, false),
    act("pinceau.fond", "Fond sous le trait du calque courant (oui / non)", true, false),
    act("pinceau.strate_fond", "Peindre dans le fond ou dans le trait (bascule)", true, false),
    act("fond.calques_choisis", "Fond sous le trait sur les calques choisis", true, false),
    act("fond.tout_le_plan", "Fond sous le trait sur tous les calques de dessin du plan", true, false),
    act("fond.retirer_choisis", "Retirer le fond sous le trait des calques choisis", true, false),
    act("fond.couleur_partout", "Couleur du projet à tous les fonds sous le trait", true, false),
    act("pinceau.remplir", "Pinceau et crayon : remplir la forme (oui / non)", true, false),
    act("outil.cutter", "Outil cutter", true, false),
    act("outil.formes", "Outil formes (dernière forme utilisée)", true, false),
    act("outil.ligne", "Outil ligne", true, false),
    act("outil.ligne_droite", "Outil ligne, droite", true, false),
    act("outil.ligne_courbe", "Outil ligne, courbe", true, false),
    act("outil.ligne_s", "Outil ligne, courbe en S", true, false),
    act("outil.rectangle", "Outil rectangle", true, false),
    act("outil.ellipse", "Outil ellipse", true, false),
    act("outil.polyligne", "Outil polyligne", true, false),
    act("outil.texte", "Outil texte", true, false),
    act("outil.perspective", "Outil déformer (enveloppe ou perspective, le dernier mode pris)", true, false),
    act("deformer.enveloppe", "Outil déformer : enveloppe de Bézier", true, false),
    act("deformer.perspective", "Outil déformer : perspective (quatre coins)", true, false),
    act("selection.reinit_pivot", "Remettre le pivot de la sélection au centre", true, false),
    act("selection.proportions", "Sélection : garder les proportions (oui / non)", true, false),
    act("outil.main", "Outil main", true, false),
    act("outil.epaisseur_crayon", "Outil épaisseur du trait de crayon", false, false),
    act("outil.taille_plus", "Pinceau plus gros", true, true),
    act("outil.taille_moins", "Pinceau plus fin", true, true),
    // Couleur (passe couleur, `colorui.rs`).
    act("couleur.intervertir", "Intervertir les couleurs d'avant-plan et d'arrière-plan", true, false),
    act("couleur.defaut", "Couleurs par défaut (noir à l'avant-plan, blanc à l'arrière-plan)", true, false),
    act("couleur.copier_code", "Copier le code hexadécimal de la couleur d'avant-plan", true, false),
    act("couleur.roue", "Roue chromatique (montrer / cacher)", true, false),
    act("couleur.roue_carre", "Roue chromatique : carré saturation / valeur", true, false),
    act("couleur.roue_triangle", "Roue chromatique : triangle saturation / valeur", true, false),
    act("couleur.curseurs_tsv", "Curseurs teinte, saturation, valeur (TSV)", true, false),
    act("couleur.curseurs_rvb", "Curseurs rouge, vert, bleu (RVB)", true, false),
    act("couleur.curseurs_tsl", "Curseurs teinte, saturation, luminosité (TSL)", true, false),
    act("couleur.recentes_effacer", "Effacer les couleurs récentes", true, false),
    act("pastille.avant_plan", "La pastille visée (menu ouvert, sinon sous la souris) devient la couleur d'avant-plan", true, false),
    act("pastille.arriere_plan", "La pastille visée (menu ouvert, sinon sous la souris) devient la couleur d'arrière-plan", true, false),
    act("pastille.renommer", "Nommer la pastille visée du nuancier (sinon celle de la couleur d'avant-plan)", true, false),
    act("pastille.remplacer", "Remplacer la pastille visée du nuancier par la couleur d'avant-plan", true, false),
    act("pastille.retirer", "Retirer du nuancier la pastille visée (sinon celle de la couleur d'avant-plan)", true, false),
    act("nuancier.ajouter", "Ajouter au nuancier la couleur d'avant-plan (ou la couleur récente visée)", true, false),
    act("nuancier.nouveau", "Nouveau nuancier", true, false),
    act("nuancier.renommer", "Renommer le nuancier montré", true, false),
    act("nuancier.dupliquer", "Dupliquer le nuancier montré", true, false),
    act("nuancier.supprimer", "Supprimer le nuancier montré (après confirmation)", true, false),
    act("nuancier.importer", "Importer un nuancier (GIMP .gpl, Photoshop .aco)", true, false),
    act("nuancier.exporter", "Exporter le nuancier montré (GIMP .gpl, Photoshop .aco)", true, false),
    act("selection.miroir_h", "Retourner la sélection horizontalement", true, false),
    act("calque.miroir_h", "Retourner le calque de gauche à droite", true, false),
    act("calque.miroir_v", "Retourner le calque de haut en bas", true, false),
    act("plan.retourner", "Retourner les plans choisis de gauche à droite", true, false),
    act("selection.miroir_v", "Retourner la sélection verticalement", true, false),
    act("dessin.supprimer_hors_selection", "Effacer ce qui est hors de la sélection", true, false),
    act("dessin.aplatir", "Aplatir le dessin", false, false),
    act("dessin.aimanter_contour", "Aimanter au contour", false, false),
    act("dessin.remplir_zones_auto", "Remplir automatiquement les zones fermées", false, false),
    act("lecture.basculer", "Lecture / pause", true, false),
    act("image.precedente", "Image précédente", true, true),
    act("image.suivante", "Image suivante", true, true),
    act("image.premiere", "Première image du plan", true, false),
    act("image.derniere", "Dernière image du plan", true, false),
    act("montage.debut", "Début du montage", true, false),
    act("montage.fin", "Fin du montage", true, false),
    act("bande.zoom_avant", "Zoom avant de la bande (le montage, ou la timeline en animation)", true, true),
    act("bande.zoom_arriere", "Zoom arrière de la bande (le montage, ou la timeline en animation)", true, true),
    act("bande.tout_voir", "Bande : tout voir (le film en storyboard, le plan en animation)", true, false),
    act("panneau.precedent", "Plan précédent", true, true),
    act("panneau.suivant", "Plan suivant", true, true),
    act("panneau.nouveau", "Nouveau plan", true, false),
    act("panneau.dupliquer", "Dupliquer le plan", true, false),
    act("scene.nouvelle", "Nouveau plan (second raccourci)", true, false),
    act("scene.dupliquer", "Dupliquer le plan (second raccourci)", true, false),
    act("scene.supprimer", "Supprimer les plans choisis (second raccourci)", true, false),
    act("scene.precedente", "Plan précédent (second raccourci)", true, true),
    act("scene.suivante", "Plan suivant (second raccourci)", true, true),
    act("sequence.nouvelle", "Nouvelle séquence", true, false),
    act("panneau.supprimer", "Supprimer les plans choisis", true, false),
    act("panneau.copier", "Copier les plans choisis", true, false),
    act("panneau.coller", "Coller les plans après le plan courant", true, false),
    act("panneau.ajouter_image", "Allonger le plan d'une image", true, true),
    act("panneau.retirer_image", "Raccourcir le plan d'une image", true, true),
    act("panneau.proprietes", "Propriétés du plan", true, false),
    act("panneau.couper", "Couper le plan à l'image courante", true, false),
    act("panneau.fusionner_suivant", "Réunir le plan et le suivant", true, false),
    act("panneau.fusionner_choisis", "Fusionner les plans choisis (qui se suivent)", true, false),
    act("scene.couper", "Couper le plan à l'image courante (comme F5)", true, false),
    act("scene.joindre_precedente", "Réunir avec le plan précédent", true, false),
    act("scene.aller_a", "Aller à un plan", true, false),
    act("marque.basculer", "Marque d'image à l'image courante (ajouter / retirer)", true, false),
    act("dessin.nouveau", "Nouveau dessin après l'exposition", true, false),
    act("dessin.dupliquer", "Dupliquer le dessin", true, false),
    act("dessin.nouvelle_cle", "Nouvelle clé à l'image courante, copie du dessin (coupe l'exposition)", true, false),
    act("dessin.nouvelle_cle_vide", "Nouvelle clé vide à l'image courante (coupe l'exposition)", true, false),
    act("dessin.supprimer", "Retirer le dessin de la timeline", true, false),
    act("exposition.allonger", "Tenir le dessin une image de plus", true, true),
    act("exposition.raccourcir", "Tenir le dessin une image de moins", true, true),
    act("calque.nouveau_vectoriel", "Nouveau calque vectoriel", true, false),
    act("calque.nouveau_bitmap", "Nouveau calque bitmap", true, false),
    act("calque.copier", "Copier le calque", true, false),
    act("calque.coller", "Coller le calque", true, false),
    act("calque.supprimer", "Supprimer les calques choisis", true, false),
    act("calque.flou", "Flou sur le calque (effet Flou gaussien, cocher / décocher)", true, false),
    act("calque.verrouiller", "Verrouiller le calque", true, false),
    act("calque.fusionner_dessous", "Fusionner avec le calque du dessous", true, false),
    act("calque.reinit_transformation", "Annuler la transformation du calque", true, false),
    act("calque.dessus", "Calque du dessus", true, true),
    act("calque.dessous", "Calque du dessous", true, true),
    act("calque.nouveau", "Nouveau calque… (nom, sorte, groupe, mode, place)", true, false),
    act("calque.dupliquer", "Dupliquer les calques choisis", true, false),
    act("calque.supprimer_vides", "Supprimer les calques vides", true, false),
    act("calque.supprimer_caches", "Supprimer les calques cachés", true, false),
    act("calque.fusionner_choisis", "Fusionner les calques choisis", true, false),
    act("calque.fusionner_visibles", "Fusionner les calques visibles", true, false),
    act("calque.fusionner_tout", "Fusionner tous les calques", true, false),
    act("calque.choisir_tout", "Choisir tous les calques", true, false),
    act("calque.choisir_visibles", "Choisir les calques visibles", true, false),
    act("calque.choisir_groupe", "Choisir les calques du même groupe", true, false),
    act("calque.choisir_autres_groupes", "Choisir les calques des autres groupes", true, false),
    act("calque.choisir_aucun", "Ne plus choisir de calque (le courant seul)", true, false),
    act("calque.choisir_dessus", "Calque du dessus, en gardant le choix", true, true),
    act("calque.choisir_dessous", "Calque du dessous, en gardant le choix", true, true),
    act("calque.monter", "Monter le calque d'un rang", true, true),
    act("calque.descendre", "Descendre le calque d'un rang", true, true),
    act("calque.premier", "Premier calque (le plus bas)", true, false),
    act("calque.dernier", "Dernier calque (le plus haut)", true, false),
    act("calque.tous_visibles", "Afficher tous les calques", true, false),
    act("calque.tous_caches", "Cacher tous les calques", true, false),
    act("calque.courant_seul", "Afficher le calque courant seul", true, false),
    act("calque.pochoirs_eteindre", "Éteindre tous les pochoirs", true, false),
    act("calque.pochoirs_inverser", "Inverser tous les pochoirs", true, false),
    act("calque.pelure", "Pelure d'oignon du calque courant (allumer / éteindre)", true, false),
    act("calque.couper", "Couper le calque", true, false),
    act("calque.coller_remplacer", "Coller le calque à la place du calque courant", true, false),
    act("images.supprimer", "Supprimer les images choisies (calques choisis)", true, false),
    act("images.couper", "Couper les images choisies", true, false),
    act("images.copier", "Copier les images choisies", true, false),
    act("images.coller", "Coller les images (insérer avant l'image courante)", true, false),
    act("images.coller_remplacer", "Coller les images (remplacer les images choisies)", true, false),
    act("images.dupliquer", "Dupliquer les images choisies", true, false),
    act("images.inverser", "Inverser l'ordre des images choisies", true, false),
    act("images.inserer_avant_calques", "Insérer une image avant (calques choisis)", true, false),
    act("images.inserer_apres_calques", "Insérer une image après (calques choisis)", true, false),
    act("images.ajouter", "Ajouter une image au plan, après l'image courante", true, true),
    act("images.inserer", "Insérer une image dans le plan, avant l'image courante", true, true),
    act("images.supprimer_plan", "Supprimer l'image du plan (tous les calques)", true, true),
    act("images.tout_choisir", "Choisir toutes les images du plan", true, false),
    act("instance.precedente", "Instance précédente (début du dessin d'avant)", true, true),
    act("instance.suivante", "Instance suivante (début du dessin d'après)", true, true),
    act("cle.precedente", "Clé précédente (début de dessin d'avant, tous les calques)", true, true),
    act("cle.suivante", "Clé suivante (début de dessin d'après, tous les calques)", true, true),
    act("instance.tete", "Tête de l'instance (sa première image)", true, false),
    act("instance.queue", "Queue de l'instance (sa dernière image)", true, false),
    act("instance.casser", "Casser l'instance à l'image courante", true, false),
    act("instance.ajouter_avant", "Nouveau dessin avant l'instance courante", true, false),
    act("instance.exposition_plus", "Une exposition de plus à chaque instance choisie", true, false),
    act("instance.exposition_moins", "Une exposition de moins à chaque instance choisie", true, false),
    act("images.faux_fixe_boucle", "Faux-fixe : rejouer les images choisies en boucle", true, false),
    act("images.faux_fixe_va_et_vient", "Faux-fixe : rejouer les images choisies en va-et-vient", true, false),
    act("images.faux_fixe_hasard", "Faux-fixe : rejouer les images choisies au hasard", true, false),
    act("calque.etirer_tenir", "Étirer le calque jusqu'à la fin du plan (tenir sa dernière image)", true, false),
    act("calque.etirer_boucle", "Étirer le calque jusqu'à la fin du plan (en boucle)", true, false),
    act("calque.etirer_va_et_vient", "Étirer le calque jusqu'à la fin du plan (en va-et-vient)", true, false),
    act("calque.couper_apres", "Couper le calque après l'image courante", true, false),
    act("images.retourner_h", "Retourner les images choisies horizontalement", true, false),
    act("images.retourner_v", "Retourner les images choisies verticalement", true, false),
    act("images.retourner_hv", "Retourner les images choisies horizontalement et verticalement", true, false),
    act("instance.renommer", "Renommer l'instance courante", true, false),
    act("instance.effacer_noms", "Effacer les noms d'instance (calques choisis)", true, false),
    act("instance.tout_renommer", "Renommer toutes les instances 1, 2, 3…", true, false),
    act("instance.renommer_intelligemment", "Renommer intelligemment les instances (préfixe, suffixe)", true, false),
    act("signet.basculer", "Signet à l'image courante (poser / retirer)", true, false),
    act("signet.precedent", "Signet précédent", true, true),
    act("signet.suivant", "Signet suivant", true, true),
    act("signets.enlever_choisis", "Enlever les signets des images choisies", true, false),
    act("signets.tout_enlever", "Enlever tous les signets du plan", true, false),
    act("entree.poser", "Point d'entrée à l'image courante", true, false),
    act("entree.basculer", "Point d'entrée (poser / retirer)", true, false),
    act("sortie.poser", "Point de sortie à l'image courante", true, false),
    act("sortie.basculer", "Point de sortie (poser / retirer)", true, false),
    act("entree_sortie.enlever", "Enlever les points d'entrée et de sortie", true, false),
    act("boucle.limites_choisies", "Les images choisies deviennent les limites de la lecture", true, false),
    act("boucle.images_choisies", "Boucle entre les images choisies (allumer / éteindre)", true, false),
    act("timeline.adapter_plan", "Timeline : adapter au plan", true, false),
    act("timeline.adapter_calque", "Timeline : adapter au calque courant", true, false),
    act("timeline.adapter_choix", "Timeline : adapter aux images choisies", true, false),
    act("timeline.adapter_image", "Timeline : adapter à l'image courante", true, false),
    act("timeline.adapter_cles", "Timeline : adapter aux clés de caméra", true, false),
    act("timeline.tout_voir", "Timeline : tout voir", true, false),
    act("marque_couleur.basculer", "Marque de couleur sur l'instance courante (poser / retirer)", true, false),
    act("marque_couleur.retirer", "Retirer la marque de couleur de l'instance courante", true, false),
    act("marque_couleur.toutes", "Marque de couleur sur toutes les instances du calque", true, false),
    act("marque_couleur.non_marquees", "Marque de couleur sur les instances non marquées", true, false),
    act("marque_couleur.effacer_calque", "Retirer les marques de couleur du calque", true, false),
    act("marque_couleur.effacer_plan", "Retirer les marques de couleur du plan", true, false),
    act("marque_couleur.precedente", "Marque de couleur précédente", true, true),
    act("marque_couleur.suivante", "Marque de couleur suivante", true, true),
    act("marque_couleur.precedente_couleur", "Marque précédente de la couleur choisie", true, true),
    act("marque_couleur.suivante_couleur", "Marque suivante de la couleur choisie", true, true),
    act("onglet.notes", "Onglet Notes de la timeline (ouvrir / fermer)", true, false),
    act("onglet.son", "Onglet Son de la timeline (ouvrir / fermer)", true, false),
    act("onglet.groupes", "Onglet Groupes de la timeline (ouvrir / fermer)", true, false),
    act("onglet.arriere_plan", "Onglet Arrière-plan de la timeline (ouvrir / fermer)", true, false),
    act("onglet.image_depart", "Onglet Image de départ de la timeline (ouvrir / fermer)", true, false),
    act("fond.aucun", "Arrière-plan : aucun (transparent)", true, false),
    act("fond.couleur", "Arrière-plan : couleur", true, false),
    act("fond.damier", "Arrière-plan : damier", true, false),
    act("plan.nouveau_avant", "Nouveau plan avant le plan courant", true, false),
    act("plan.avancer", "Avancer le plan (plus tôt dans le montage)", true, false),
    act("plan.reculer", "Reculer le plan (plus tard dans le montage)", true, false),
    act("lecture.boucle", "Lecture en boucle (oui / non)", true, false),
    act("storyboard.verrouiller", "Verrouiller / déverrouiller le storyboard", true, false),
    act("projet.proprietes", "Réglages du projet (propriétés, résolution, cadence)", true, false),
    act("projet.nouveau_depuis_plans", "Nouveau projet à partir des plans choisis", true, false),
    act("pelure.panneau", "Panneau de la pelure d'oignon (ouvrir / fermer)", true, false),
    act("pelure.devant", "Pelure d'oignon devant le dessin (oui / non)", true, false),
    act("pelure.lier", "Lier les curseurs d'opacité de la pelure (oui / non)", true, false),
    act("pelure.activer_tout", "Pelure : montrer toutes les images", true, false),
    act("pelure.desactiver_tout", "Pelure : cacher toutes les images", true, false),
    act("pelure.mode_instance", "Pelure : mode instance", true, false),
    act("pelure.mode_image", "Pelure : mode image", true, false),
    act("pelure.mode_signet", "Pelure : mode signet", true, false),
    act("pelure.mode_marque", "Pelure : mode marque de couleur", true, false),
    act("pelure.decaler", "Décaler les images de la pelure (glisser sur le dessin)", true, false),
    act("pelure.remettre", "Remettre les images de la pelure en place", true, false),
    act("vue.feuille_exposition", "Panneau Feuille d'exposition (ouvrir / fermer)", true, false),
    act("vue.historique", "Panneau Historique des annulations (ouvrir / fermer)", true, false),
    // Commentaires sur la timeline (`commentairesui.rs`).
    act("vue.commentaires", "Panneau Commentaires (ouvrir / fermer)", true, false),
    act("commentaire.ajouter", "Nouveau commentaire (à l'image courante)", true, false),
    act("commentaire.resoudre", "Commentaire choisi : résolu / rouvert", true, false),
    act("commentaire.supprimer", "Supprimer le commentaire choisi", true, false),
    act("commentaire.precedent", "Aller au commentaire précédent", true, false),
    act("commentaire.suivant", "Aller au commentaire suivant", true, false),
    // Barre des panneaux (`barrepanneaux.rs`) : ouvrir, mettre devant ou fermer un panneau de côté.
    act("vue.reglages", "Panneau Réglages de l'outil (ouvrir / fermer)", true, false),
    act("vue.pinceaux", "Panneau Pinceaux (ouvrir / fermer)", true, false),
    act("vue.couleur", "Panneau Couleur (ouvrir / fermer)", true, false),
    act("vue.guides", "Panneau Guides (ouvrir / fermer)", true, false),
    act("vue.calques", "Panneau Calques (ouvrir / fermer)", true, false),
    act("vue.fiche", "Panneau Fiche du plan (ouvrir / fermer)", true, false),
    act("vue.barre_panneaux", "Barre des panneaux, sur le bord droit (montrer / cacher)", true, false),
    act("role.cle", "Rôle des images visées : clé", true, false),
    act("role.breakdown", "Rôle des images visées : breakdown", true, false),
    act("role.intervalle", "Rôle des images visées : intervalle à faire", true, false),
    act("role.intervalle_complete", "Rôle des images visées : intervalle complété", true, false),
    act("role.intervalle_special", "Rôle des images visées : intervalle spécial à faire", true, false),
    act("role.intervalle_special_complete", "Rôle des images visées : intervalle spécial complété", true, false),
    act("role.aucun", "Retirer le rôle des images visées", true, false),
    act("role.timeline", "Montrer / cacher les rôles dans la timeline", true, false),
    act("role.calque_courant", "Rôles sur le calque courant seulement (oui / non)", true, false),
    act("flip.panneau", "Panneau des flips (ouvrir / fermer)", true, false),
    act("flip.jouer", "Flipper avec le flip courant (maintenir)", true, false),
    act("flip.precedent", "Flip manuel : image précédente du flip", true, true),
    act("flip.suivant", "Flip manuel : image suivante du flip", true, true),
    act("flip.changer", "Passer au flip suivant de la liste", true, false),
    act("flip.cles", "Flipper les clés (maintenir)", true, false),
    act("flip.breakdowns", "Flipper les breakdowns (maintenir)", true, false),
    act("flip.intervalles", "Flipper les intervalles à faire (maintenir)", true, false),
    act("flip.intervalles_completes", "Flipper les intervalles complétés (maintenir)", true, false),
    act("flip.speciaux", "Flipper les intervalles spéciaux à faire (maintenir)", true, false),
    act("flip.speciaux_completes", "Flipper les intervalles spéciaux complétés (maintenir)", true, false),
    act("fx.panneau", "Pile d'effets du calque (ouvrir / fermer)", true, false),
    act("fx.cle_creer", "Créer une clé de l'effet choisi à l'image courante", true, false),
    act("fx.cle_supprimer", "Supprimer la clé de l'effet choisi à l'image courante", true, false),
    act("fx.cle_auto", "Création automatique des clés d'effets (oui / non)", true, false),
    act("fx.cles_timeline", "Clés d'effets dans la timeline et le montage (montrer / cacher)", true, false),
    act("fx.choisir_aucun", "Ne choisir aucun effet (les clés d'effets se cachent dans la timeline et le montage)", true, false),
    act("fx.cles_copier", "Copier les clés d'effets choisies", true, false),
    act("fx.cles_coller", "Coller les clés d'effets à l'image courante du calque courant", true, false),
    act("fx.cles_supprimer", "Supprimer les clés d'effets choisies (rien sans clé choisie)", true, false),
    act("fx.tremblement", "Tremblement du trait du calque courant (calque vectoriel : oui / non)", true, false),
    act("fx.tremblement_repos", "Tremblement du trait : animer aussi à l'arrêt (oui / non)", true, false),
    act("calque.nouveau_ctg", "Nouveau calque CTG", true, false),
    act("calque.transformer_ctg", "Transformer en calque CTG", true, false),
    act("calque.desactiver_ctg", "Désactiver le calque CTG", true, false),
    act("ctg.charger_structure", "Calque CTG : charger la structure à partir des sources", true, false),
    act("ctg.charger_structure_effacer", "Calque CTG : charger la structure à partir des sources (effacer)", true, false),
    act("fichier.importer_video", "Importer une vidéo (calque vidéo)", true, false),
    act("fichier.importer_sboard", "Importer un projet Storyboard Pro", true, false),
    act("fichier.exporter_sboard", "Exporter vers Storyboard Pro", true, false),
    act("pinceau.enregistrer_traits", "Enregistrer mes traits (banc d'essai du moteur de dessin)", true, false),
    act("fichier.importer_sequence", "Importer une séquence d'images (calque vidéo)", true, false),
    act("calque.video_en_animation", "Transformer le calque vidéo en calque d'animation", true, false),
    act("calque.propager", "Propager la peinture du calque en suivant une vidéo ou une animation (façon EbSynth)", true, false),
    act("images.intermediaire", "Insérer une image intermédiaire (entre le dessin courant et le suivant)", true, false),
    act("images.intermediaires", "Insérer des images intermédiaires…", true, false),
    act("images.intervalles", "Remplir les images vides par des intervalles", true, false),
    act("images.etirer_interpoler", "Étirer des images en interpolant…", true, false),
    act("projet.changer_cadence", "Changer la cadence du projet en gardant les durées…", true, false),
    act("dossier.nouveau", "Nouveau dossier de calques", true, false),
    act("dossier.ranger", "Placer les calques choisis dans un nouveau dossier", true, false),
    act("dossier.deposer", "Déposer les calques choisis dans le dossier", true, false),
    act("dossier.sortir", "Sortir les calques choisis de leur dossier", true, false),
    act("dossier.supprimer_seul", "Supprimer uniquement le dossier (ses calques restent)", true, false),
    act("dossier.joindre", "Joindre les dossiers choisis dans le dossier courant", true, false),
    act("groupes.editer", "Groupes de calques (noms, couleurs, montrés dans la timeline)", true, false),
    act("groupes.focus", "Focus sur le groupe du calque courant (la timeline ne montre que lui)", true, false),
    act("outil.camera", "Outil caméra", true, false),
    act("outil.transformer", "Outil transformer le calque", true, false),
    act("camera.vue", "Vue à travers la caméra", true, false),
    act("camera.masque", "Masque hors-champ de la caméra", true, false),
    act("camera.reinitialiser", "Réinitialiser la caméra", true, false),
    act("camera.cle_debut", "Image clé de caméra au début du plan", true, false),
    act("camera.cle_ici", "Image clé de caméra à l'image courante", true, false),
    act("camera.cle_fin", "Image clé de caméra à la fin du plan", true, false),
    act("camera.cle_supprimer", "Retirer l'image clé de caméra de l'image courante", true, false),
    act("camera.cle_adoucir", "Adoucir le mouvement à cette image clé", true, false),
    act("camera.cles_copier", "Copier les clés de caméra choisies", true, false),
    act("camera.cles_coller", "Coller les clés de caméra à l'image courante", true, false),
    act("camera.cles_supprimer", "Supprimer les clés de caméra choisies (rien sans clé choisie)", true, false),
    act("camera.cles_tout_choisir", "Choisir toutes les clés de caméra du plan", true, false),
    act("camera.effet_appliquer", "Appliquer l'effet de caméra choisi (tremblement, explosion, zoom…)", true, false),
    act("cadrage.tiers", "Aide au cadrage : règle des tiers (grille 3 × 3)", true, false),
    act("cadrage.grille_or", "Aide au cadrage : grille d'or (phi), points aux intersections", true, false),
    act("cadrage.spirale", "Aide au cadrage : spirale d'or et ses carrés", true, false),
    act("cadrage.spirale_orientation", "Aide au cadrage : orientation suivante de la spirale d'or (8)", true, false),
    act("cadrage.diagonales", "Aide au cadrage : diagonales et réciproques (symétrie dynamique)", true, false),
    act("cadrage.croix", "Aide au cadrage : croix centrale", true, false),
    act("cadrage.aucune", "Aides au cadrage : toutes éteintes", true, false),
    act("cadrage.format_suivant", "Aide au cadrage : cadre de format suivant (16:9, 2,39:1…)", true, false),
    act("vue.table_lumineuse", "Table lumineuse (autres calques estompés)", true, false),
    act("vue.dessus", "Vue de dessus (profondeur des calques)", true, false),
    act("vue.cadres_securite", "Zones de sécurité (action, titres)", true, false),
    act("vue.3d", "Plateau en 3D", true, false),
    act("vue3d.recadrer", "Plateau en 3D : recadrer sur tout le plateau", true, false),
    act("outil3d.orbite", "Plateau en 3D : tourner autour", true, false),
    act("outil3d.glisser", "Plateau en 3D : glisser la vue", true, false),
    act("outil3d.avancer", "Plateau en 3D : avancer / reculer", true, false),
    act("outil3d.deplacer", "Plateau en 3D : déplacer un calque ou la caméra", true, false),
    act("outil.repere3d", "Outil repère 3D (tourner autour du modèle)", true, false),
    act("fichier.importer_modele3d", "Importer un modèle 3D (glTF) comme repère", true, false),
    act("calque.nouveau_fond", "Nouveau calque de fond (tout en bas, tenu sur tout le plan)", true, false),
    act("espace.standard", "Espace de travail : Standard", true, false),
    act("espace.dessin", "Espace de travail : Dessin", true, false),
    act("espace.timeline", "Espace de travail : Timeline", true, false),
    act("espace.ensemble", "Espace de travail : Vue d'ensemble", true, false),
    act("espace.enregistrer", "Enregistrer la disposition comme espace de travail", true, false),
    act("vue.panneau_outils", "Afficher / masquer le panneau Outils", true, false),
    act("vue.panneau_fiche", "Afficher / masquer la fiche et les calques", true, false),
    act("vue.panneau_bande", "Afficher / masquer le montage ou la timeline", true, false),
    act("vue.reajuster_panneaux", "Réajuster les panneaux à leur contenu", true, false),
    act("vue.performance", "Performance : superposition des temps (images, panneaux, arrière-plan)", true, false),
    act("guide.activer", "Guide de perspective : afficher et utiliser", true, false),
    act("guide.aligner", "Guide : aligner les traits", true, false),
    act("guide.verrouiller", "Guide : verrouiller", true, false),
    act("guide.lignes_reference", "Guide : lignes de référence", true, false),
    act("guide.gomme", "Guide : aligner aussi la gomme", true, false),
    act("guide.curseur_complet", "Guide : lignes du curseur jusqu'au bord", true, false),
    act("son.importer", "Importer un son", true, false),
    act("son.nouvelle_piste", "Nouvelle piste son", true, false),
    act("son.couper", "Couper le son à l'image courante", true, false),
    act("son.defilement", "Son au défilement (activer / couper)", true, false),
    act("son.lecture", "Son pendant la lecture (activer / couper)", true, false),
    act("reference.decharger", "Décharger la référence courante", false, false),
    act("legende.chercher", "Chercher dans les légendes", false, false),
];

pub fn action_info(id: &str) -> Option<&'static ActionInfo> {
    ACTIONS.iter().find(|a| a.id == id)
}

/// Comment reconnaître la touche.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyMatch {
    /// Le caractère tapé : la lettre écrite sur la touche, quelle que soit la disposition du clavier.
    Char(Key),
    /// La position de la touche, nommée d'après un clavier QWERTY.
    Pos(Key),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Combo {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub key: KeyMatch,
    /// Maj indifférente (pour « + », que l'on tape avec ou sans Maj selon le clavier).
    pub any_shift: bool,
}

impl Combo {
    pub fn matches(&self, key: Key, physical: Option<Key>, m: Modifiers) -> bool {
        if m.command != self.ctrl || m.alt != self.alt || (!self.any_shift && m.shift != self.shift) {
            return false;
        }
        match self.key {
            KeyMatch::Char(k) => key == k,
            KeyMatch::Pos(k) => physical == Some(k),
        }
    }

    pub fn label(&self) -> String {
        let mut s = String::new();
        if self.ctrl {
            s += "Ctrl+";
        }
        if self.shift && !self.any_shift {
            s += "Maj+";
        }
        if self.alt {
            s += "Alt+";
        }
        s += &key_label(self.key, self.any_shift);
        s
    }
}

fn key_label(k: KeyMatch, plus: bool) -> String {
    if plus {
        return "+".into();
    }
    match k {
        KeyMatch::Char(key) => match key {
            Key::Enter => "Entrée".into(),
            Key::Space => "Espace".into(),
            Key::Delete => "Suppr".into(),
            Key::Backspace => "Retour arrière".into(),
            Key::Escape => "Échap".into(),
            Key::Insert => "Inser".into(),
            Key::Home => "Origine".into(),
            Key::End => "Fin".into(),
            Key::PageUp => "Pg préc.".into(),
            Key::PageDown => "Pg suiv.".into(),
            Key::ArrowLeft => "Flèche gauche".into(),
            Key::ArrowRight => "Flèche droite".into(),
            Key::ArrowUp => "Flèche haut".into(),
            Key::ArrowDown => "Flèche bas".into(),
            other => other.name().to_string(),
        },
        // Touche repérée par sa position : on montre ce qui est écrit dessus en AZERTY.
        KeyMatch::Pos(key) => match (azerty_glyph(key), qwerty_glyph(key)) {
            (Some(a), Some(q)) if a != q => format!("{a} (QWERTY {q})"),
            (Some(a), _) => a.to_string(),
            _ => key.name().to_string(),
        },
    }
}

/// Ce qui est écrit sur la touche à cette position, sur un clavier AZERTY français.
pub fn azerty_glyph(k: Key) -> Option<&'static str> {
    Some(match k {
        Key::Comma => ";",
        Key::Period => ":",
        Key::Semicolon => "M",
        Key::Quote => "ù",
        Key::OpenBracket => "^",
        Key::CloseBracket => "$",
        Key::Minus => ")",
        Key::Equals => "=",
        Key::Slash => "!",
        Key::Backslash => "*",
        Key::Backtick => "²",
        Key::Plus => "+",
        _ => return digit(k),
    })
}

pub fn qwerty_glyph(k: Key) -> Option<&'static str> {
    Some(match k {
        Key::Comma => ",",
        Key::Period => ".",
        Key::Semicolon => ";",
        Key::Quote => "'",
        Key::OpenBracket => "[",
        Key::CloseBracket => "]",
        Key::Minus => "-",
        Key::Equals => "=",
        Key::Slash => "/",
        Key::Backslash => "\\",
        Key::Backtick => "`",
        Key::Plus => "+",
        _ => return digit(k),
    })
}

fn digit(k: Key) -> Option<&'static str> {
    Some(match k {
        Key::Num0 => "0",
        Key::Num1 => "1",
        Key::Num2 => "2",
        Key::Num3 => "3",
        Key::Num4 => "4",
        Key::Num5 => "5",
        Key::Num6 => "6",
        Key::Num7 => "7",
        Key::Num8 => "8",
        Key::Num9 => "9",
        _ => return None,
    })
}

/// Le raccourci tapé (`key` : la touche lue selon la disposition du clavier ; `physical` : sa
/// position), écrit comme dans le fichier (« Ctrl+Maj+K », « Alt+F5 », « Maj+1 »). Les lettres
/// et les touches nommées sont reconnues par ce qu'elles écrivent, les chiffres et les signes
/// par leur position. Rien pour Échap (il sert à annuler) ni pour une touche inconnue du fichier.
pub fn combo_text(key: Key, physical: Option<Key>, m: Modifiers) -> Option<String> {
    let name = key.name();
    let named = match key {
        Key::Escape => return None,
        Key::Enter => "Entrée",
        Key::Space => "Espace",
        Key::Delete => "Suppr",
        Key::Backspace => "Retour arrière",
        Key::Tab => "Tab",
        Key::Insert => "Inser",
        Key::Home => "Origine",
        Key::End => "Fin",
        Key::PageUp => "PgPréc",
        Key::PageDown => "PgSuiv",
        Key::ArrowLeft => "Gauche",
        Key::ArrowRight => "Droite",
        Key::ArrowUp => "Haut",
        Key::ArrowDown => "Bas",
        _ => "",
    };
    let text = if !named.is_empty() {
        named.to_string()
    } else if name.len() == 1 && name.chars().all(|c| c.is_ascii_alphabetic()) {
        name.to_string()
    } else if name.len() > 1 && name.starts_with('F') && name[1..].chars().all(|c| c.is_ascii_digit()) {
        name.to_string()
    } else {
        qwerty_glyph(physical.unwrap_or(key))?.to_string()
    };
    let mut s = String::new();
    if m.command || m.ctrl {
        s += "Ctrl+";
    }
    if m.shift {
        s += "Maj+";
    }
    if m.alt {
        s += "Alt+";
    }
    s += &text;
    // Ce que le fichier sait relire seulement.
    parse_combo(&s).ok().map(|_| s)
}

/// Lit une touche seule (« A », « F5 », « Entrée », « [ », « pos:, »…).
/// Renvoie plusieurs façons de la reconnaître quand il y en a plusieurs (« + »).
fn parse_key(name: &str) -> Option<(Vec<KeyMatch>, bool)> {
    let (force, name) = if let Some(rest) = strip_prefix_ci(name, "pos:") {
        (Some(true), rest)
    } else if let Some(rest) = strip_prefix_ci(name, "car:") {
        (Some(false), rest)
    } else {
        (None, name)
    };
    let lower = name.trim().to_lowercase();
    let named = match lower.as_str() {
        "entrée" | "entree" | "return" | "enter" => Some(Key::Enter),
        "espace" | "space" => Some(Key::Space),
        "suppr" | "delete" | "del" | "supprimer" => Some(Key::Delete),
        "retour arrière" | "retour arriere" | "backspace" => Some(Key::Backspace),
        "échap" | "echap" | "esc" | "escape" => Some(Key::Escape),
        "tab" => Some(Key::Tab),
        "inser" | "insert" | "ins" => Some(Key::Insert),
        "origine" | "home" => Some(Key::Home),
        "fin" | "end" => Some(Key::End),
        "pgpréc" | "pgprec" | "pg préc." | "pageup" => Some(Key::PageUp),
        "pgsuiv" | "pg suiv." | "pagedown" => Some(Key::PageDown),
        "gauche" | "flèche gauche" | "left" => Some(Key::ArrowLeft),
        "droite" | "flèche droite" | "right" => Some(Key::ArrowRight),
        "haut" | "flèche haut" | "up" => Some(Key::ArrowUp),
        "bas" | "flèche bas" | "down" => Some(Key::ArrowDown),
        _ => None,
    };
    let make = |k: Key, default_pos: bool| {
        if force.unwrap_or(default_pos) {
            KeyMatch::Pos(k)
        } else {
            KeyMatch::Char(k)
        }
    };
    if let Some(k) = named {
        return Some((vec![make(k, false)], false));
    }
    if lower.len() >= 2 && lower.starts_with('f') && lower[1..].chars().all(|c| c.is_ascii_digit()) {
        return Key::from_name(&name.trim().to_uppercase()).map(|k| (vec![make(k, false)], false));
    }
    let mut chars = name.trim().chars();
    let c = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    if c.is_ascii_alphabetic() {
        let k = Key::from_name(&c.to_ascii_uppercase().to_string())?;
        return Some((vec![make(k, false)], false));
    }
    if c == '+' {
        // La touche « =/+ » du clavier principal ou le + du pavé numérique, Maj indifférente.
        return Some((vec![KeyMatch::Pos(Key::Equals), KeyMatch::Pos(Key::Plus)], true));
    }
    let k = match c {
        '0' => Key::Num0,
        '1' => Key::Num1,
        '2' => Key::Num2,
        '3' => Key::Num3,
        '4' => Key::Num4,
        '5' => Key::Num5,
        '6' => Key::Num6,
        '7' => Key::Num7,
        '8' => Key::Num8,
        '9' => Key::Num9,
        ',' => Key::Comma,
        '.' => Key::Period,
        ';' => Key::Semicolon,
        ':' => Key::Colon,
        '\'' => Key::Quote,
        '[' => Key::OpenBracket,
        ']' => Key::CloseBracket,
        '-' => Key::Minus,
        '=' => Key::Equals,
        '/' => Key::Slash,
        '\\' => Key::Backslash,
        '`' => Key::Backtick,
        _ => return None,
    };
    Some((vec![make(k, true)], false))
}

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    (s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix)).then(|| &s[prefix.len()..])
}

/// Lit un raccourci complet (« Ctrl+Maj+Z », « Alt+B », « + », « Ctrl++ »).
pub fn parse_combo(text: &str) -> Result<Vec<Combo>, String> {
    let s = text.trim();
    let (mods, key) = if s == "+" {
        ("", "+")
    } else if let Some(m) = s.strip_suffix("++") {
        (m, "+")
    } else {
        match s.rfind('+') {
            Some(i) => (&s[..i], &s[i + 1..]),
            None => ("", s),
        }
    };
    let (mut ctrl, mut shift, mut alt) = (false, false, false);
    for m in mods.split('+').map(str::trim).filter(|m| !m.is_empty()) {
        match m.to_lowercase().as_str() {
            "ctrl" | "control" | "contrôle" | "cmd" => ctrl = true,
            "maj" | "shift" => shift = true,
            "alt" => alt = true,
            other => return Err(format!("modificateur inconnu « {other} » dans « {text} »")),
        }
    }
    let (keys, any_shift) = parse_key(key).ok_or_else(|| format!("touche inconnue dans « {text} »"))?;
    Ok(keys
        .into_iter()
        .map(|key| Combo {
            ctrl,
            shift,
            alt,
            key,
            any_shift,
        })
        .collect())
}

#[derive(Clone, Debug)]
pub struct Binding {
    pub action: String,
    /// Les touches telles qu'écrites dans le fichier.
    pub keys: Vec<String>,
    pub combos: Vec<Combo>,
    pub context: Option<Mode>,
    pub active: bool,
    /// Marqué « non implémenté » dans le fichier.
    pub reserved: bool,
    pub source: String,
    /// Vient du fichier personnel (il remplace le raccourci par défaut).
    pub user: bool,
}

#[derive(Deserialize)]
struct FileFormat {
    version: u32,
    #[serde(default)]
    raccourci: Vec<Entry>,
}

#[derive(Deserialize, Serialize, Clone)]
struct Entry {
    action: String,
    #[serde(default)]
    touches: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    contexte: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    etat: Option<String>,
}

fn parse_file(text: &str, warnings: &mut Vec<String>) -> Result<Vec<Binding>, String> {
    let file: FileFormat = toml::from_str(text).map_err(|e| e.to_string())?;
    if file.version != 1 {
        warnings.push(format!("version de fichier {} inconnue (attendue : 1)", file.version));
    }
    let mut out = Vec::new();
    for e in file.raccourci {
        let info = action_info(&e.action);
        if info.is_none() {
            warnings.push(format!("action inconnue « {} » (ignorée)", e.action));
            continue;
        }
        let context = match e.contexte.as_deref().map(str::to_lowercase).as_deref() {
            None | Some("") | Some("partout") => None,
            Some("storyboard") => Some(Mode::Storyboard),
            Some("animation") => Some(Mode::Animation),
            Some(other) => {
                warnings.push(format!("contexte inconnu « {other} » pour {}", e.action));
                None
            }
        };
        let etat = e.etat.as_deref().map(str::to_lowercase).unwrap_or_default();
        let reserved = etat.contains("non impl");
        let disabled = etat.contains("désactiv") || etat.contains("desactiv");
        let implemented = info.is_some_and(|i| i.implemented);
        let mut combos = Vec::new();
        for k in &e.touches {
            match parse_combo(k) {
                Ok(c) => combos.extend(c),
                // Un raccourci en réserve peut viser un geste qu'on ne sait pas encore lire.
                Err(err) if !reserved => warnings.push(format!("{} : {err}", e.action)),
                Err(_) => {}
            }
        }
        if !reserved && !implemented {
            warnings.push(format!("« {} » n'existe pas encore : raccourci gardé en réserve", e.action));
        }
        out.push(Binding {
            action: e.action,
            keys: e.touches,
            combos,
            context,
            active: implemented && !reserved && !disabled,
            reserved: reserved || !implemented,
            source: e.source,
            user: false,
        });
    }
    Ok(out)
}

fn contexts_overlap(a: Option<Mode>, b: Option<Mode>) -> bool {
    a.is_none() || b.is_none() || a == b
}

#[derive(Default)]
pub struct Keymap {
    pub bindings: Vec<Binding>,
    pub warnings: Vec<String>,
    /// Fichier utilisateur pris en compte, le cas échéant.
    pub user_file: Option<PathBuf>,
}

impl Keymap {
    /// Raccourcis par défaut, puis ceux du fichier utilisateur qui remplacent action par action.
    pub fn from_texts(default: &str, user: Option<&str>) -> Keymap {
        let mut warnings = Vec::new();
        let mut bindings = match parse_file(default, &mut warnings) {
            Ok(b) => b,
            Err(e) => {
                warnings.push(format!("fichier par défaut illisible : {e}"));
                Vec::new()
            }
        };
        if let Some(text) = user {
            match parse_file(text, &mut warnings) {
                Ok(mut user_bindings) => {
                    for b in &mut user_bindings {
                        b.user = true;
                    }
                    let redefined: Vec<String> = user_bindings.iter().map(|b| b.action.clone()).collect();
                    bindings.retain(|b| !redefined.contains(&b.action));
                    // Placés devant : en cas de touche partagée, le fichier personnel l'emporte.
                    bindings.splice(0..0, user_bindings);
                }
                Err(e) => warnings.push(format!("fichier de raccourcis illisible, raccourcis par défaut utilisés : {e}")),
            }
        }
        let mut map = Keymap {
            bindings,
            warnings,
            user_file: None,
        };
        map.check_conflicts();
        map
    }

    /// Charge le fichier à côté de l'exécutable, sinon celui de %APPDATA%\SHOT, sinon le défaut.
    pub fn load() -> Keymap {
        let path = Self::candidate_paths().into_iter().find(|p| p.is_file());
        let user = path.as_ref().and_then(|p| std::fs::read_to_string(p).ok());
        let mut map = Keymap::from_texts(DEFAULT_KEYMAP, user.as_deref());
        map.user_file = path;
        map
    }

    fn candidate_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();
        if let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())) {
            paths.push(dir.join("keymap.toml"));
        }
        if let Some(p) = Self::appdata_path() {
            paths.push(p);
        }
        paths
    }

    pub fn appdata_path() -> Option<PathBuf> {
        crate::donnees::roaming().map(|d| d.join("keymap.toml"))
    }

    fn check_conflicts(&mut self) {
        let active: Vec<&Binding> = self.bindings.iter().filter(|b| b.active).collect();
        for (i, a) in active.iter().enumerate() {
            for b in &active[i + 1..] {
                if a.action == b.action || !contexts_overlap(a.context, b.context) {
                    continue;
                }
                if let Some(c) = a.combos.iter().find(|c| b.combos.contains(c)) {
                    self.warnings.push(format!(
                        "conflit : {} et {} utilisent tous deux {}",
                        a.action,
                        b.action,
                        c.label()
                    ));
                }
            }
        }
    }

    /// Action déclenchée par cette touche dans ce mode.
    pub fn find(&self, key: Key, physical: Option<Key>, m: Modifiers, mode: Mode) -> Option<&str> {
        self.bindings
            .iter()
            .filter(|b| b.active && b.context.is_none_or(|c| c == mode))
            .find(|b| b.combos.iter().any(|c| c.matches(key, physical, m)))
            .map(|b| b.action.as_str())
    }

    /// Raccourci à afficher dans les menus (le premier défini pour l'action).
    pub fn label(&self, action: &str) -> String {
        self.bindings
            .iter()
            .filter(|b| b.active && b.action == action)
            .flat_map(|b| b.combos.first())
            .next()
            .map(Combo::label)
            .unwrap_or_default()
    }

    /// Touches d'une action, telles qu'écrites dans le fichier qui la définit.
    pub fn keys_of(&self, action: &str) -> Vec<String> {
        self.bindings.iter().filter(|b| b.action == action).flat_map(|b| b.keys.clone()).collect()
    }

    /// Tous les raccourcis lisibles d'une action (« Ctrl+K », « F5 »…), sans doublon.
    pub fn labels_of(&self, action: &str) -> Vec<String> {
        let mut v: Vec<String> = self
            .bindings
            .iter()
            .filter(|b| b.active && b.action == action)
            .flat_map(|b| b.combos.iter().map(Combo::label))
            .collect();
        v.dedup();
        v
    }

    /// D'où vient le raccourci de l'action (le champ `source` du fichier).
    pub fn source_of(&self, action: &str) -> &str {
        self.bindings.iter().find(|b| b.action == action).map_or("", |b| b.source.as_str())
    }

    /// Vrai si le fichier personnel redéfinit l'action.
    pub fn is_user(&self, action: &str) -> bool {
        self.bindings.iter().any(|b| b.action == action && b.user)
    }

    /// Contexte (mode) où joue le raccourci de l'action ; partout s'il n'en a pas.
    pub fn context_of(&self, action: &str) -> Option<Mode> {
        self.bindings.iter().find(|b| b.action == action).and_then(|b| b.context)
    }

    /// Les autres actions actives que le raccourci `text` déclenche déjà là où joue `action`.
    pub fn conflicts(&self, text: &str, action: &str) -> Vec<String> {
        let Ok(combos) = parse_combo(text) else { return Vec::new() };
        let context = self.context_of(action);
        let mut out: Vec<String> = self
            .bindings
            .iter()
            .filter(|b| b.active && b.action != action && contexts_overlap(b.context, context))
            .filter(|b| b.combos.iter().any(|c| combos.contains(c)))
            .map(|b| b.action.clone())
            .collect();
        out.dedup();
        out
    }

    /// Texte du fichier personnel après ces changements : à chaque action, ses nouvelles touches
    /// (une liste vide : plus de raccourci), ou `None` pour revenir au raccourci par défaut. Le
    /// reste du fichier `current` est gardé, sauf ses commentaires.
    pub fn edited_user_file(&self, current: Option<&str>, changes: &[(String, Option<Vec<String>>)]) -> Result<String, String> {
        let mut entries: Vec<Entry> = match current {
            Some(text) if !text.trim().is_empty() => {
                toml::from_str::<FileFormat>(text).map_err(|e| format!("fichier de raccourcis illisible : {e}"))?.raccourci
            }
            _ => Vec::new(),
        };
        for (action, keys) in changes {
            // Le contexte suit celui du raccourci actuel (par défaut ou personnel).
            let contexte = match self.context_of(action) {
                Some(Mode::Storyboard) => Some("storyboard".to_string()),
                Some(Mode::Animation) => Some("animation".to_string()),
                None => None,
            };
            entries.retain(|e| &e.action != action);
            if let Some(keys) = keys {
                entries.push(Entry {
                    action: action.clone(),
                    touches: keys.clone(),
                    source: "choisi dans SHOT".into(),
                    contexte,
                    etat: None,
                });
            }
        }
        #[derive(Serialize)]
        struct Out<'a> {
            version: u32,
            raccourci: &'a [Entry],
        }
        let body = toml::to_string(&Out { version: 1, raccourci: &entries }).map_err(|e| e.to_string())?;
        Ok(format!(
            "# Raccourcis personnels de SHOT : chacun remplace, action par action, celui du fichier\n\
             # par défaut. Écrit par SHOT (clic droit sur une commande > « Assigner un raccourci »,\n\
             # ou Fichier > Raccourcis clavier) ; on peut aussi le modifier à la main.\n\n{body}"
        ))
    }

    /// Les changements qui donnent `text` à `action` (en plus de ses touches si `add`), et le
    /// retirent des actions qui l'avaient déjà.
    pub fn assign_changes(&self, action: &str, text: &str, add: bool) -> Vec<(String, Option<Vec<String>>)> {
        let mut keys = if add { self.keys_of(action) } else { Vec::new() };
        if !keys.iter().any(|k| k == text) {
            keys.push(text.to_string());
        }
        let mut changes = vec![(action.to_string(), Some(keys))];
        let combos = parse_combo(text).unwrap_or_default();
        for other in self.conflicts(text, action) {
            let rest: Vec<String> = self
                .keys_of(&other)
                .into_iter()
                .filter(|k| !parse_combo(k).unwrap_or_default().iter().any(|c| combos.contains(c)))
                .collect();
            changes.push((other, Some(rest)));
        }
        changes
    }

    /// Touche de l'action actuellement maintenue (gestes « maintenir », comme Espace pour la main).
    pub fn held(&self, input: &eframe::egui::InputState, action: &str) -> bool {
        self.bindings
            .iter()
            .filter(|b| b.active && b.action == action)
            .flat_map(|b| b.combos.iter())
            .any(|c| match c.key {
                KeyMatch::Char(k) => input.key_down(k) && input.modifiers.command == c.ctrl && input.modifiers.alt == c.alt,
                KeyMatch::Pos(_) => false,
            })
    }
}
