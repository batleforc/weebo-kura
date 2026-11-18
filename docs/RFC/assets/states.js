// Fills every <span class="state" data-s="…"> with its matrix and word (the
// Three Channel Rule): ok = fine 9 lit, held = coarse 3, pending = coarse 2,
// signed = coarse 1, blocked = coarse 4 in crimson.
const lit = { ok: ["fine", 9, "s-ok"], held: ["coarse", 3, "s-held"], pending: ["coarse", 2, "s-pending"], signed: ["coarse", 1, "s-pending"], blocked: ["coarse", 4, "s-blocked"] };
for (const el of document.querySelectorAll(".state")) {
  const [kind, on, cls] = lit[el.dataset.s] ?? lit.pending;
  const cells = kind === "fine" ? 9 : 4;
  const m = document.createElement("span");
  m.className = `m ${kind}`;
  m.setAttribute("aria-hidden", "true");
  for (let i = 0; i < cells; i += 1) m.append(Object.assign(document.createElement("i"), { className: i < on ? "on" : "" }));
  el.classList.add(cls);
  el.prepend(m);
}
