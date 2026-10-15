// Pelure Compagnon : le storyboard d'un projet Pelure, en React. Toutes les données viennent des
// commandes Rust (api.ts) ; après chaque changement, Rust renvoie le projet entier (`ResumeProjet`)
// et `rev` augmente pour refaire les vignettes.
import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { api, type ResumePanneau, type ResumeProjet } from "./api";

const LARGEUR_VIGNETTE = 256;

function Vignette({ index, rev }: { index: number; rev: number }) {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    let actif = true;
    let url: string | null = null;
    api
      .vignette(index, LARGEUR_VIGNETTE)
      .then((u) => {
        if (actif) {
          url = u;
          setSrc(u);
        } else {
          URL.revokeObjectURL(u);
        }
      })
      .catch(() => setSrc(null));
    return () => {
      actif = false;
      if (url) URL.revokeObjectURL(url);
    };
  }, [index, rev]);
  return src ? (
    <img src={src} alt="" draggable={false} className="block w-full bg-white" />
  ) : (
    <div className="aspect-video w-full animate-pulse bg-neutral-700" />
  );
}

function Bouton(props: { onClick: () => void; disabled?: boolean; titre?: string; children: ReactNode }) {
  return (
    <button
      onClick={props.onClick}
      disabled={props.disabled}
      title={props.titre}
      className="rounded bg-neutral-700 px-3 py-1.5 text-sm hover:bg-neutral-600 disabled:cursor-not-allowed disabled:opacity-40"
    >
      {props.children}
    </button>
  );
}

function Fiche(props: {
  panneau: ResumePanneau;
  total: number;
  fps: number;
  /** Lance une commande ; `ensuite` : le panneau à choisir après. */
  agir: (f: () => Promise<ResumeProjet>, ensuite?: number) => void;
}) {
  const { panneau: p, total, fps, agir } = props;
  const [action, setAction] = useState(p.action);
  const [dialogue, setDialogue] = useState(p.dialogue);
  const [notes, setNotes] = useState(p.notes);
  const [images, setImages] = useState(String(p.images));
  // Un autre panneau, ou le même changé par ailleurs : on reprend ses valeurs.
  useEffect(() => {
    setAction(p.action);
    setDialogue(p.dialogue);
    setNotes(p.notes);
    setImages(String(p.images));
  }, [p.id, p.action, p.dialogue, p.notes, p.images]);

  const textes = () => {
    if (action !== p.action || dialogue !== p.dialogue || notes !== p.notes) {
      agir(() => api.textes(p.index, action, dialogue, notes));
    }
  };
  const duree = () => {
    const n = Math.round(Number(images));
    if (Number.isFinite(n) && n >= 1 && n !== p.images) agir(() => api.duree(p.index, n));
    else setImages(String(p.images));
  };
  const champ = "w-full rounded border border-neutral-600 bg-neutral-900 p-2 text-sm focus:border-sky-500 focus:outline-none";
  return (
    <aside className="flex w-80 shrink-0 flex-col gap-3 overflow-y-auto border-l border-neutral-700 bg-neutral-900 p-4">
      <div>
        <div className="text-lg font-semibold">{p.titre}</div>
        <div className="text-xs text-neutral-400">
          début {p.debut} · {p.calques} calque{p.calques > 1 ? "s" : ""}
        </div>
      </div>
      <label className="flex items-center gap-2 text-sm">
        Durée
        <input
          className={champ + " w-24"}
          type="number"
          min={1}
          value={images}
          onChange={(e) => setImages(e.target.value)}
          onBlur={duree}
          onKeyDown={(e) => e.key === "Enter" && duree()}
        />
        <span className="text-neutral-400">images ({p.duree}, {fps} i/s)</span>
      </label>
      {(
        [
          ["Action", action, setAction],
          ["Dialogue", dialogue, setDialogue],
          ["Notes", notes, setNotes],
        ] as const
      ).map(([nom, valeur, changer]) => (
        <label key={nom} className="flex flex-col gap-1 text-sm">
          {nom}
          <textarea className={champ + " h-24 resize-y"} value={valeur} onChange={(e) => changer(e.target.value)} onBlur={textes} />
        </label>
      ))}
      <div className="flex flex-wrap gap-2">
        <Bouton onClick={() => agir(() => api.ajouter(p.index), p.index + 1)} titre="Nouveau panneau après celui-ci">
          + Panneau
        </Bouton>
        <Bouton onClick={() => agir(() => api.dupliquer(p.index), p.index + 1)}>Dupliquer</Bouton>
        <Bouton onClick={() => agir(() => api.deplacer(p.index, p.index - 1), p.index - 1)} disabled={p.index === 0} titre="Avancer d'une place">
          ←
        </Bouton>
        <Bouton onClick={() => agir(() => api.deplacer(p.index, p.index + 2), p.index + 1)} disabled={p.index >= total - 1} titre="Reculer d'une place">
          →
        </Bouton>
        <Bouton onClick={() => agir(() => api.supprimer(p.index), Math.max(0, p.index - 1))} disabled={total <= 1}>
          Supprimer
        </Bouton>
      </div>
    </aside>
  );
}

export default function App() {
  const [projet, setProjet] = useState<ResumeProjet | null>(null);
  const [choisi, setChoisi] = useState(0);
  const [rev, setRev] = useState(0);
  const [message, setMessage] = useState<string | null>(null);
  const [tire, setTire] = useState<number | null>(null);

  // Lance une commande qui renvoie le projet ; une erreur s'affiche en bas.
  const agir = useCallback((f: () => Promise<ResumeProjet>, ensuite?: number) => {
    f()
      .then((p) => {
        setProjet(p);
        setRev((r) => r + 1);
        if (ensuite !== undefined) setChoisi(ensuite);
      })
      .catch((e) => setMessage(String(e)));
  }, []);

  const ouvrir = () =>
    api
      .ouvrir()
      .then((o) => {
        if (!o) return;
        setProjet(o.projet);
        setChoisi(0);
        setRev((r) => r + 1);
        setMessage(o.avertissements.length ? o.avertissements.join(" · ") : `« ${o.projet.nom} » ouvert.`);
      })
      .catch((e) => setMessage(String(e)));

  const enregistrer = () =>
    agir(async () => {
      const p = await api.enregistrer();
      setMessage("Enregistré.");
      return p;
    });

  // Projet ouvert au lancement (passé en argument) : on l'affiche tout de suite.
  useEffect(() => {
    api
      .resume()
      .then((p) => {
        setProjet(p);
        setRev((r) => r + 1);
      })
      .catch(() => {});
  }, []);

  // Ctrl+S : enregistrer.
  useEffect(() => {
    const touche = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s" && projet?.chemin) {
        e.preventDefault();
        enregistrer();
      }
    };
    window.addEventListener("keydown", touche);
    return () => window.removeEventListener("keydown", touche);
  });

  const panneaux = useMemo(
    () => projet?.sequences.flatMap((s) => s.scenes.flatMap((sc) => sc.panneaux)) ?? [],
    [projet],
  );
  const courant = panneaux[Math.min(choisi, panneaux.length - 1)];

  if (!projet) {
    return (
      <main className="flex h-full flex-col items-center justify-center gap-6">
        <h1 className="text-3xl font-semibold">Pelure — compagnon</h1>
        <p className="max-w-md text-center text-neutral-400">
          Le storyboard d'un projet Pelure : panneaux, textes, durées, ordre, export PDF. Le dessin se fait dans Pelure.
        </p>
        <div className="flex gap-3">
          <Bouton onClick={ouvrir}>Ouvrir un projet…</Bouton>
          <Bouton onClick={() => agir(() => api.nouveau(1920, 1080, 24))}>Nouveau (1920×1080, 24 i/s)</Bouton>
        </div>
        {message && <p className="text-sm text-amber-400">{message}</p>}
      </main>
    );
  }

  return (
    <div className="flex h-full flex-col">
      <header className="flex items-center gap-2 border-b border-neutral-700 bg-neutral-800 px-4 py-2">
        <span className="mr-2 font-semibold">
          {projet.nom}
          {projet.modifie ? " •" : ""}
        </span>
        <span className="mr-auto text-xs text-neutral-400">
          {projet.cadre[0]}×{projet.cadre[1]} · {projet.fps} i/s · {panneaux.length} panneaux · {projet.duree}
        </span>
        <Bouton onClick={ouvrir}>Ouvrir…</Bouton>
        <Bouton onClick={enregistrer} disabled={!projet.chemin} titre={projet.chemin ?? "Projet neuf : enregistre-le d'abord dans Pelure"}>
          Enregistrer
        </Bouton>
        <Bouton
          onClick={() =>
            api
              .exporterPdf()
              .then((c) => c && setMessage(`PDF écrit : ${c}`))
              .catch((e) => setMessage(String(e)))
          }
        >
          Exporter le PDF…
        </Bouton>
      </header>
      <div className="flex min-h-0 flex-1">
        <main className="min-w-0 flex-1 overflow-y-auto p-4">
          {projet.sequences.map((seq) => (
            <section key={seq.id} className="mb-6">
              <h2 className="mb-2 text-sm font-semibold uppercase tracking-wide text-neutral-400">Séquence {seq.nom}</h2>
              {seq.scenes.map((sc) => (
                <div key={sc.id} className="mb-4">
                  <h3 className="mb-2 text-sm text-sky-300">Scène {sc.nom}</h3>
                  <div className="grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-3">
                    {sc.panneaux.map((p) => (
                      <button
                        key={p.id}
                        draggable
                        onDragStart={() => setTire(p.index)}
                        onDragOver={(e) => e.preventDefault()}
                        onDrop={() => {
                          if (tire !== null && tire !== p.index) {
                            // Lâché sur un panneau : le tiré prend sa place (avant lui, ou après s'il venait de gauche).
                            agir(() => api.deplacer(tire, tire < p.index ? p.index + 1 : p.index), p.index);
                          }
                          setTire(null);
                        }}
                        onClick={() => setChoisi(p.index)}
                        className={
                          "overflow-hidden rounded border-2 bg-neutral-800 text-left " +
                          (courant?.id === p.id ? "border-sky-500" : "border-transparent hover:border-neutral-500")
                        }
                      >
                        <Vignette index={p.index} rev={rev} />
                        <div className="flex justify-between px-2 py-1 text-xs">
                          <span className="font-semibold">{p.titre}</span>
                          <span className="text-neutral-400">{p.duree}</span>
                        </div>
                        {p.action && <div className="truncate px-2 pb-1 text-xs text-neutral-400">{p.action}</div>}
                      </button>
                    ))}
                  </div>
                </div>
              ))}
            </section>
          ))}
        </main>
        {courant && <Fiche panneau={courant} total={panneaux.length} fps={projet.fps} agir={agir} />}
      </div>
      {message && (
        <footer className="flex items-center justify-between border-t border-neutral-700 bg-neutral-800 px-4 py-1 text-xs text-amber-300">
          <span>{message}</span>
          <button onClick={() => setMessage(null)} className="text-neutral-400 hover:text-white">
            ✕
          </button>
        </footer>
      )}
    </div>
  );
}
