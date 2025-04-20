var _a;
import { F as x, L as j, a6 as S, a7 as A, a8 as F, a9 as k, M as G, aa as J, ab as T, m as K, ac as Q, n as c, H, ad as I, x as E, w as L, B as u, ae as w, af as X, ag as Z, ah as ee, o as W, ai as M, aj as R, ak as te, al as re, am as ae, an as ne, ao as oe, ap as ie, z as se, p as ue, c as le, j as de } from "./DEAb5m-A.js";
function we(e) {
  return e.endsWith("capture") && e !== "gotpointercapture" && e !== "lostpointercapture";
}
const fe = ["beforeinput", "click", "change", "dblclick", "contextmenu", "focusin", "focusout", "input", "keydown", "keyup", "mousedown", "mousemove", "mouseout", "mouseover", "mouseup", "pointerdown", "pointermove", "pointerout", "pointerover", "pointerup", "touchend", "touchmove", "touchstart"];
function be(e) {
  return fe.includes(e);
}
const ce = { formnovalidate: "formNoValidate", ismap: "isMap", nomodule: "noModule", playsinline: "playsInline", readonly: "readOnly", defaultvalue: "defaultValue", defaultchecked: "defaultChecked", srcobject: "srcObject", novalidate: "noValidate", allowfullscreen: "allowFullscreen", disablepictureinpicture: "disablePictureInPicture", disableremoteplayback: "disableRemotePlayback" };
function Te(e) {
  return e = e.toLowerCase(), ce[e] ?? e;
}
const _e = ["touchstart", "touchmove"];
function ve(e) {
  return _e.includes(e);
}
function Le(e, t) {
  if (t) {
    const r = document.body;
    e.autofocus = true, x(() => {
      document.activeElement === r && e.focus();
    });
  }
}
let C = false;
function pe() {
  C || (C = true, document.addEventListener("reset", (e) => {
    Promise.resolve().then(() => {
      var _a2;
      if (!e.defaultPrevented) for (const t of e.target.elements) (_a2 = t.__on_r) == null ? void 0 : _a2.call(t);
    });
  }, { capture: true }));
}
function ke(e, t, r, o = true) {
  o && r();
  for (var n of t) e.addEventListener(n, r);
  j(() => {
    for (var a of t) e.removeEventListener(a, r);
  });
}
function B(e) {
  var t = F, r = k;
  S(null), A(null);
  try {
    return e();
  } finally {
    S(t), A(r);
  }
}
function Ne(e, t, r, o = r) {
  e.addEventListener(t, () => B(r));
  const n = e.__on_r;
  n ? e.__on_r = () => {
    n(), o(true);
  } : e.__on_r = () => o(true), pe();
}
const $ = /* @__PURE__ */ new Set(), O = /* @__PURE__ */ new Set();
function he(e, t, r, o = {}) {
  function n(a) {
    if (o.capture || b.call(t, a), !a.cancelBubble) return B(() => r == null ? void 0 : r.call(this, a));
  }
  return e.startsWith("pointer") || e.startsWith("touch") || e === "wheel" ? x(() => {
    t.addEventListener(e, n, o);
  }) : t.addEventListener(e, n, o), n;
}
function Se(e, t, r, o, n) {
  var a = { capture: o, passive: n }, i = he(e, t, r, a);
  (t === document.body || t === window || t === document) && j(() => {
    t.removeEventListener(e, i, a);
  });
}
function Ae(e) {
  for (var t = 0; t < e.length; t++) $.add(e[t]);
  for (var r of O) r(e);
}
function b(e) {
  var _a2;
  var t = this, r = t.ownerDocument, o = e.type, n = ((_a2 = e.composedPath) == null ? void 0 : _a2.call(e)) || [], a = n[0] || e.target, i = 0, _ = e.__root;
  if (_) {
    var d = n.indexOf(_);
    if (d !== -1 && (t === document || t === window)) {
      e.__root = t;
      return;
    }
    var m = n.indexOf(t);
    if (m === -1) return;
    d <= m && (i = d);
  }
  if (a = n[i] || e.target, a !== t) {
    G(e, "currentTarget", { configurable: true, get() {
      return a || r;
    } });
    var P = F, v = k;
    S(null), A(null);
    try {
      for (var s, l = []; a !== null; ) {
        var y = a.assignedSlot || a.parentNode || a.host || null;
        try {
          var h = a["__" + o];
          if (h != null && (!a.disabled || e.target === a)) if (J(h)) {
            var [Y, ...z] = h;
            Y.apply(a, [e, ...z]);
          } else h.call(a, e);
        } catch (N) {
          s ? l.push(N) : s = N;
        }
        if (e.cancelBubble || y === t || y === null) break;
        a = y;
      }
      if (s) {
        for (let N of l) queueMicrotask(() => {
          throw N;
        });
        throw s;
      }
    } finally {
      e.__root = t, delete e.currentTarget, S(P), A(v);
    }
  }
}
let f;
function me() {
  f = void 0;
}
function Pe(e) {
  let t = null, r = c;
  var o;
  if (c) {
    for (t = u, f === void 0 && (f = w(document.head)); f !== null && (f.nodeType !== 8 || f.data !== H); ) f = I(f);
    f === null ? E(false) : f = L(I(f));
  }
  c || (o = document.head.appendChild(T()));
  try {
    K(() => e(o), Q);
  } finally {
    r && (E(true), f = u, L(t));
  }
}
function q(e) {
  var t = document.createElement("template");
  return t.innerHTML = e, t.content;
}
function p(e, t) {
  var r = k;
  r.nodes_start === null && (r.nodes_start = e, r.nodes_end = t);
}
function Re(e, t) {
  var r = (t & Z) !== 0, o = (t & ee) !== 0, n, a = !e.startsWith("<!>");
  return () => {
    if (c) return p(u, null), u;
    n === void 0 && (n = q(a ? e : "<!>" + e), r || (n = w(n)));
    var i = o || X ? document.importNode(n, true) : n.cloneNode(true);
    if (r) {
      var _ = w(i), d = i.lastChild;
      p(_, d);
    } else p(i, i);
    return i;
  };
}
function Ie(e, t, r = "svg") {
  var o = !e.startsWith("<!>"), n = `<${r}>${o ? e : "<!>" + e}</${r}>`, a;
  return () => {
    if (c) return p(u, null), u;
    if (!a) {
      var i = q(n), _ = w(i);
      a = w(_);
    }
    var d = a.cloneNode(true);
    return p(d, d), d;
  };
}
function Me(e = "") {
  if (!c) {
    var t = T(e + "");
    return p(t, t), t;
  }
  var r = u;
  return r.nodeType !== 3 && (r.before(r = T()), L(r)), p(r, r), r;
}
function Oe() {
  if (c) return p(u, null), u;
  var e = document.createDocumentFragment(), t = document.createComment(""), r = T();
  return e.append(t, r), p(t, r), e;
}
function De(e, t) {
  if (c) {
    k.nodes_end = u, W();
    return;
  }
  e !== null && e.before(t);
}
let V = true;
function Ce(e, t) {
  var r = t == null ? "" : typeof t == "object" ? t + "" : t;
  r !== (e.__t ?? (e.__t = e.nodeValue)) && (e.__t = r, e.nodeValue = r + "");
}
function ye(e, t) {
  return U(e, t);
}
function Ve(e, t) {
  M(), t.intro = t.intro ?? false;
  const r = t.target, o = c, n = u;
  try {
    for (var a = w(r); a && (a.nodeType !== 8 || a.data !== H); ) a = I(a);
    if (!a) throw R;
    E(true), L(a), W();
    const i = U(e, { ...t, anchor: a });
    if (u === null || u.nodeType !== 8 || u.data !== te) throw re(), R;
    return E(false), i;
  } catch (i) {
    if (i === R) return t.recover === false && ae(), M(), ne(r), E(false), ye(e, t);
    throw i;
  } finally {
    E(o), L(n), me();
  }
}
const g = /* @__PURE__ */ new Map();
function U(e, { target: t, anchor: r, props: o = {}, events: n, context: a, intro: i = true }) {
  M();
  var _ = /* @__PURE__ */ new Set(), d = (v) => {
    for (var s = 0; s < v.length; s++) {
      var l = v[s];
      if (!_.has(l)) {
        _.add(l);
        var y = ve(l);
        t.addEventListener(l, b, { passive: y });
        var h = g.get(l);
        h === void 0 ? (document.addEventListener(l, b, { passive: y }), g.set(l, 1)) : g.set(l, h + 1);
      }
    }
  };
  d(oe($)), O.add(d);
  var m = void 0, P = ie(() => {
    var v = r ?? t.appendChild(T());
    return se(() => {
      if (a) {
        ue({});
        var s = le;
        s.c = a;
      }
      n && (o.$$events = n), c && p(v, null), V = i, m = e(v, o) || {}, V = true, c && (k.nodes_end = u), a && de();
    }), () => {
      var _a2;
      for (var s of _) {
        t.removeEventListener(s, b);
        var l = g.get(s);
        --l === 0 ? (document.removeEventListener(s, b), g.delete(s)) : g.set(s, l);
      }
      O.delete(d), v !== r && ((_a2 = v.parentNode) == null ? void 0 : _a2.removeChild(v));
    };
  });
  return D.set(m, P), m;
}
let D = /* @__PURE__ */ new WeakMap();
function xe(e, t) {
  const r = D.get(e);
  return r ? (D.delete(e), r(t)) : Promise.resolve();
}
const ge = "5";
typeof window < "u" && ((_a = window.__svelte ?? (window.__svelte = {})).v ?? (_a.v = /* @__PURE__ */ new Set())).add(ge);
export {
  De as a,
  Me as b,
  Oe as c,
  he as d,
  Ae as e,
  Le as f,
  be as g,
  Ve as h,
  we as i,
  pe as j,
  V as k,
  Ne as l,
  ye as m,
  Te as n,
  Ie as o,
  Se as p,
  Pe as q,
  ke as r,
  Ce as s,
  Re as t,
  xe as u,
  B as w
};
