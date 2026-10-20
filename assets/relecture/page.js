(function () {
  'use strict';
  /* Page de relecture SHOT : aucune requête, aucun script ni service du dehors. Le texte du projet n'est
     jamais écrit dans ce script : il est lu dans la page (attributs data-*). */
  var body = document.body;
  var PAQUET = body.getAttribute('data-paquet') || '';
  var PROJET = body.getAttribute('data-projet') || '';
  var FICHIER = body.getAttribute('data-fichier') || 'notes';
  var CLE = 'shot-relecture-' + PAQUET;
  var COULEURS = { rouge: '#d6261c', bleu: '#1f64d8', vert: '#1a8a3c' };
  var NOMS_COULEURS = { rouge: 'Rouge', bleu: 'Bleu', vert: 'Vert' };
  var TRAIT = 4;
  var GOMME = 24;

  var nomChamp = document.getElementById('relecteur');
  var general = document.getElementById('general');
  var message = document.getElementById('message');
  var compte = document.getElementById('compte');
  var secours = document.getElementById('secours');
  var cartes = [];
  var minuterie = null;

  function el(nom, classe, texte) {
    var e = document.createElement(nom);
    if (classe) { e.className = classe; }
    if (texte) { e.textContent = texte; }
    return e;
  }

  function dire(texte, erreur) {
    message.textContent = texte;
    message.className = 'message' + (erreur ? ' erreur' : '');
  }

  /* ---------------------------------------------------------------- le dessin sur une image */
  function Dessin(cadre) {
    var w = parseInt(cadre.getAttribute('data-w'), 10) || 960;
    var h = parseInt(cadre.getAttribute('data-h'), 10) || 540;
    var canvas = document.createElement('canvas');
    canvas.width = w;
    canvas.height = h;
    canvas.className = 'calque';
    cadre.appendChild(canvas);
    var ctx = canvas.getContext('2d');
    var traits = [];
    var courant = null;
    var self = { traits: traits, outil: null, couleur: 'rouge', surchange: function () {} };

    function style(t) {
      ctx.globalCompositeOperation = t.g ? 'destination-out' : 'source-over';
      ctx.strokeStyle = t.g ? '#000' : COULEURS[t.c] || COULEURS.rouge;
      ctx.fillStyle = ctx.strokeStyle;
      ctx.lineWidth = t.l;
      ctx.lineCap = 'round';
      ctx.lineJoin = 'round';
    }
    function segment(t, a, b) {
      style(t);
      ctx.beginPath();
      ctx.moveTo(a[0], a[1]);
      ctx.lineTo(b[0], b[1]);
      ctx.stroke();
    }
    function point(t, a) {
      style(t);
      ctx.beginPath();
      ctx.arc(a[0], a[1], t.l / 2, 0, 2 * Math.PI);
      ctx.fill();
    }
    function tracer(t) {
      if (t.p.length === 1) { point(t, t.p[0]); return; }
      for (var i = 1; i < t.p.length; i++) { segment(t, t.p[i - 1], t.p[i]); }
    }
    function redessiner() {
      ctx.clearRect(0, 0, w, h);
      traits.forEach(tracer);
    }
    function position(e) {
      var r = canvas.getBoundingClientRect();
      return [Math.round((e.clientX - r.left) * w / r.width * 10) / 10, Math.round((e.clientY - r.top) * h / r.height * 10) / 10];
    }
    canvas.addEventListener('pointerdown', function (e) {
      if (!self.outil) { return; }
      e.preventDefault();
      try { canvas.setPointerCapture(e.pointerId); } catch (x) { /* sans capture, le trait suit quand même */ }
      var gomme = self.outil === 'gomme';
      courant = { c: self.couleur, g: gomme, l: gomme ? GOMME : TRAIT, p: [position(e)] };
      traits.push(courant);
      point(courant, courant.p[0]);
    });
    canvas.addEventListener('pointermove', function (e) {
      if (!courant) { return; }
      e.preventDefault();
      var p = position(e);
      var d = courant.p[courant.p.length - 1];
      if (Math.abs(p[0] - d[0]) + Math.abs(p[1] - d[1]) < 0.8) { return; }
      courant.p.push(p);
      segment(courant, d, p);
    });
    function fin() {
      if (!courant) { return; }
      courant = null;
      self.surchange();
    }
    canvas.addEventListener('pointerup', fin);
    canvas.addEventListener('pointercancel', fin);
    canvas.addEventListener('contextmenu', function (e) { if (self.outil) { e.preventDefault(); } });

    self.annuler = function () { traits.pop(); redessiner(); self.surchange(); };
    self.effacer = function () { traits.length = 0; redessiner(); self.surchange(); };
    self.remplir = function (liste) {
      traits.length = 0;
      (liste || []).forEach(function (t) {
        if (t && t.p && t.p.length && t.l > 0 && t.l <= 80) { traits.push({ c: COULEURS[t.c] ? t.c : 'rouge', g: !!t.g, l: +t.l, p: t.p }); }
      });
      redessiner();
    };
    /* Il y a quelque chose de dessiné : au moins un trait de couleur, et pas tout gommé. */
    self.aDuTrait = function () {
      if (!traits.some(function (t) { return !t.g; })) { return false; }
      var d = ctx.getImageData(0, 0, w, h).data;
      for (var i = 3; i < d.length; i += 4) { if (d[i] > 0) { return true; } }
      return false;
    };
    self.png = function () { return canvas.toDataURL('image/png'); };
    return self;
  }

  /* ---------------------------------------------------------------- une carte (une image du plan) */
  function Carte(section) {
    var cadre = section.querySelector('.cadre');
    var outils = section.querySelector('.outils');
    var texte = section.querySelector('.note');
    var d = Dessin(cadre);
    var c = {
      cle: section.getAttribute('data-plan') + '-' + section.getAttribute('data-image'),
      plan: section.getAttribute('data-plan'),
      image: section.getAttribute('data-image'),
      numero: section.getAttribute('data-numero') || '',
      titre: section.getAttribute('data-titre') || '',
      texte: texte,
      dessin: d
    };
    var bDessiner = el('button', 'bouton calme petit', 'Dessiner');
    bDessiner.type = 'button';
    bDessiner.setAttribute('aria-pressed', 'false');
    bDessiner.title = 'Dessiner par-dessus l’image avec le doigt, le stylet ou la souris';
    var groupe = [];
    function choisir(outil, couleur) {
      d.outil = outil;
      if (couleur) { d.couleur = couleur; }
      cadre.classList.toggle('dessine', !!outil);
      bDessiner.setAttribute('aria-pressed', outil === 'stylo' ? 'true' : 'false');
      bGomme.setAttribute('aria-pressed', outil === 'gomme' ? 'true' : 'false');
      groupe.forEach(function (b) { b.setAttribute('aria-pressed', outil === 'stylo' && b.getAttribute('data-c') === d.couleur ? 'true' : 'false'); });
    }
    var bGomme = el('button', 'bouton calme petit', 'Gomme');
    bGomme.type = 'button';
    bGomme.setAttribute('aria-pressed', 'false');
    bDessiner.addEventListener('click', function () { choisir(d.outil === 'stylo' ? null : 'stylo'); });
    bGomme.addEventListener('click', function () { choisir(d.outil === 'gomme' ? null : 'gomme'); });
    outils.appendChild(bDessiner);
    Object.keys(COULEURS).forEach(function (nom) {
      var b = el('button', 'couleur ' + nom);
      b.type = 'button';
      b.setAttribute('data-c', nom);
      b.setAttribute('aria-label', NOMS_COULEURS[nom]);
      b.title = NOMS_COULEURS[nom];
      b.setAttribute('aria-pressed', 'false');
      b.addEventListener('click', function () { choisir('stylo', nom); });
      groupe.push(b);
      outils.appendChild(b);
    });
    outils.appendChild(bGomme);
    var s = el('span', 'sep');
    outils.appendChild(s);
    var bAnnuler = el('button', 'bouton calme petit', 'Annuler le trait');
    bAnnuler.type = 'button';
    bAnnuler.addEventListener('click', function () { d.annuler(); });
    var bEffacer = el('button', 'bouton calme petit', 'Tout effacer');
    bEffacer.type = 'button';
    bEffacer.addEventListener('click', function () { if (!d.traits.length || window.confirm('Effacer le dessin de cette image ?')) { d.effacer(); } });
    outils.appendChild(bAnnuler);
    outils.appendChild(bEffacer);
    texte.addEventListener('input', planifier);
    d.surchange = planifier;
    return c;
  }

  /* ---------------------------------------------------------------- les notes */
  function collecter() {
    var notes = [];
    cartes.forEach(function (c) {
      var t = c.texte.value.trim();
      var png = c.dessin.aDuTrait() ? c.dessin.png() : null;
      if (t || png) {
        var n = { id: PAQUET + '-' + c.cle, plan: c.plan, image: c.image, numero: c.numero, titre: c.titre, texte: t };
        if (png) { n.dessin = png; }
        notes.push(n);
      }
    });
    return notes;
  }

  function nombre() {
    var n = 0;
    cartes.forEach(function (c) { if (c.texte.value.trim() || c.dessin.traits.some(function (t) { return !t.g; })) { n++; } });
    if (general.value.trim()) { n++; }
    return n;
  }

  function majCompte() {
    var n = nombre();
    compte.textContent = n === 0 ? 'Aucune note' : (n === 1 ? '1 note' : n + ' notes');
  }

  function planifier() {
    majCompte();
    if (minuterie) { clearTimeout(minuterie); }
    minuterie = setTimeout(brouillon, 700);
  }

  /* Brouillon dans ce navigateur seulement (si le navigateur le permet) : on retrouve son travail en rouvrant la page. */
  function brouillon() {
    try {
      var o = { nom: nomChamp.value, general: general.value, cartes: {} };
      cartes.forEach(function (c) {
        var t = c.texte.value;
        if (t || c.dessin.traits.length) { o.cartes[c.cle] = { t: t, s: c.dessin.traits }; }
      });
      window.localStorage.setItem(CLE, JSON.stringify(o));
    } catch (e) { /* pas de stockage : tant pis, le bouton Enregistrer reste la vraie sauvegarde */ }
  }

  function restaurer() {
    try {
      var brut = window.localStorage.getItem(CLE);
      if (!brut) { return; }
      var o = JSON.parse(brut);
      if (typeof o.nom === 'string') { nomChamp.value = o.nom; }
      if (typeof o.general === 'string') { general.value = o.general; }
      cartes.forEach(function (c) {
        var x = o.cartes && o.cartes[c.cle];
        if (x) {
          if (typeof x.t === 'string') { c.texte.value = x.t; }
          if (Array.isArray(x.s)) { c.dessin.remplir(x.s); }
        }
      });
      if (nombre() > 0) { dire('Vos notes de la dernière fois ont été retrouvées dans ce navigateur.', false); }
    } catch (e) { /* brouillon illisible : on repart de zéro */ }
  }

  function date2(n) { return (n < 10 ? '0' : '') + n; }

  function enregistrer() {
    var nom = nomChamp.value.trim();
    nomChamp.classList.toggle('manque', !nom);
    if (!nom) {
      dire('Écrivez votre nom en haut de la page, pour que le réalisateur sache de qui viennent les notes.', true);
      nomChamp.focus();
      return;
    }
    var notes = collecter();
    if (!notes.length && !general.value.trim()) {
      dire('Il n’y a encore aucune note à enregistrer.', true);
      return;
    }
    var maintenant = new Date();
    var donnees = {
      format: 'shot-relecture',
      version: 1,
      paquet: PAQUET,
      projet: PROJET,
      relecteur: nom,
      cree: Math.floor(maintenant.getTime() / 1000),
      date: maintenant.toISOString(),
      general: general.value.trim(),
      notes: notes
    };
    var json = JSON.stringify(donnees);
    var jour = maintenant.getFullYear() + '-' + date2(maintenant.getMonth() + 1) + '-' + date2(maintenant.getDate());
    var fichier = FICHIER + '-' + jour + '.json';
    try {
      var blob = new Blob([json], { type: 'application/json' });
      var url = URL.createObjectURL(blob);
      var a = document.createElement('a');
      a.href = url;
      a.download = fichier;
      a.style.display = 'none';
      document.body.appendChild(a);
      a.click();
      setTimeout(function () { URL.revokeObjectURL(url); a.remove(); }, 4000);
      dire('Fichier « ' + fichier + ' » enregistré (' + notes.length + (notes.length > 1 ? ' notes' : ' note') + '). Envoyez-le par e-mail au réalisateur.', false);
    } catch (e) {
      dire('Le téléchargement n’a pas démarré : copiez le texte du cadre ci-dessous dans un fichier « ' + fichier + ' ».', true);
    }
    secours.hidden = false;
    secours.querySelector('textarea').value = json;
    brouillon();
  }

  /* ---------------------------------------------------------------- la lecture d'un plan */
  function Lecteur(boite) {
    var images = boite.querySelectorAll('.pile img');
    if (!images.length) { return; }
    var fps = parseInt(boite.getAttribute('data-fps'), 10) || 24;
    var bouton = boite.querySelector('button');
    var curseur = boite.querySelector('input');
    var tc = boite.querySelector('.tc');
    var i = 0;
    var minuteur = null;
    var duree = Array.prototype.map.call(images, function (m) { return Math.max(1, parseInt(m.getAttribute('data-h'), 10) || 1); });
    curseur.max = String(images.length - 1);
    function montrer(k) {
      images[i].classList.remove('on');
      i = k;
      images[i].classList.add('on');
      curseur.value = String(i);
      tc.textContent = (i + 1) + ' / ' + images.length;
    }
    function arreter() {
      if (minuteur) { clearTimeout(minuteur); minuteur = null; }
      bouton.textContent = 'Lire le plan';
    }
    function suite() {
      if (i + 1 >= images.length) { arreter(); montrer(0); return; }
      montrer(i + 1);
      minuteur = setTimeout(suite, duree[i] * 1000 / fps);
    }
    bouton.addEventListener('click', function () {
      if (minuteur) { arreter(); return; }
      if (i + 1 >= images.length) { montrer(0); }
      bouton.textContent = 'Pause';
      minuteur = setTimeout(suite, duree[i] * 1000 / fps);
    });
    curseur.addEventListener('input', function () { arreter(); montrer(parseInt(curseur.value, 10) || 0); });
    montrer(0);
  }

  /* ---------------------------------------------------------------- démarrage */
  Array.prototype.forEach.call(document.querySelectorAll('.carte'), function (s) { cartes.push(Carte(s)); });
  Array.prototype.forEach.call(document.querySelectorAll('.lecteur'), Lecteur);
  nomChamp.addEventListener('input', function () { nomChamp.classList.remove('manque'); planifier(); });
  general.addEventListener('input', planifier);
  document.getElementById('enregistrer').addEventListener('click', enregistrer);
  restaurer();
  majCompte();
})();
