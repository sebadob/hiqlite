var _a;
import { G as x, M as H, a7 as A, a8 as S, a9 as j, aa as N, N as z, ab as J, ac as T, m as K, ad as Q, n as d, H as W, ae as R, y as E, x as L, C as u, af as w, ag as X, ah as Z, ai as ee, o as F, aj as I, ak as P, al as te, am as re, an as ae, ao as ne, ap as oe, aq as ie, A as se, p as ue, c as le, j as fe } from "./CxznHt52.js";
function be(e) {
  return e.endsWith("capture") && e !== "gotpointercapture" && e !== "lostpointercapture";
}
const ce = ["beforeinput", "click", "change", "dblclick", "contextmenu", "focusin", "focusout", "input", "keydown", "keyup", "mousedown", "mousemove", "mouseout", "mouseover", "mouseup", "pointerdown", "pointermove", "pointerout", "pointerover", "pointerup", "touchend", "touchmove", "touchstart"];
function Te(e) {
  return ce.includes(e);
}
const de = { formnovalidate: "formNoValidate", ismap: "isMap", nomodule: "noModule", playsinline: "playsInline", readonly: "readOnly", defaultvalue: "defaultValue", defaultchecked: "defaultChecked", srcobject: "srcObject", novalidate: "noValidate", allowfullscreen: "allowFullscreen", disablepictureinpicture: "disablePictureInPicture", disableremoteplayback: "disableRemotePlayback" };
function Le(e) {
  return e = e.toLowerCase(), de[e] ?? e;
}
const _e = ["touchstart", "touchmove"];
function ve(e) {
  return _e.includes(e);
}
function Ne(e, t) {
  if (t) {
    const r = document.body;
    e.autofocus = true, x(() => {
      document.activeElement === r && e.focus();
    });
  }
}
let D = false;
function pe() {
  D || (D = true, document.addEventListener("reset", (e) => {
    Promise.resolve().then(() => {
      var _a2;
      if (!e.defaultPrevented) for (const t of e.target.elements) (_a2 = t.__on_r) == null ? void 0 : _a2.call(t);
    });
  }, { capture: true }));
}
function ke(e, t, r, o = true) {
  o && r();
  for (var n of t) e.addEventListener(n, r);
  H(() => {
    for (var a of t) e.removeEventListener(a, r);
  });
}
function $(e) {
  var t = j, r = N;
  A(null), S(null);
  try {
    return e();
  } finally {
    A(t), S(r);
  }
}
function Ae(e, t, r, o = r) {
  e.addEventListener(t, () => $(r));
  const n = e.__on_r;
  n ? e.__on_r = () => {
    n(), o(true);
  } : e.__on_r = () => o(true), pe();
}
const q = /* @__PURE__ */ new Set(), O = /* @__PURE__ */ new Set();
function he(e, t, r, o = {}) {
  function n(a) {
    if (o.capture || b.call(t, a), !a.cancelBubble) return $(() => r == null ? void 0 : r.call(this, a));
  }
  return e.startsWith("pointer") || e.startsWith("touch") || e === "wheel" ? x(() => {
    t.addEventListener(e, n, o);
  }) : t.addEventListener(e, n, o), n;
}
function Se(e, t, r, o, n) {
  var a = { capture: o, passive: n }, i = he(e, t, r, a);
  (t === document.body || t === window || t === document || t instanceof HTMLMediaElement) && H(() => {
    t.removeEventListener(e, i, a);
  });
}
function Me(e) {
  for (var t = 0; t < e.length; t++) q.add(e[t]);
  for (var r of O) r(e);
}
function b(e) {
  var _a2;
  var t = this, r = t.ownerDocument, o = e.type, n = ((_a2 = e.composedPath) == null ? void 0 : _a2.call(e)) || [], a = n[0] || e.target, i = 0, _ = e.__root;
  if (_) {
    var f = n.indexOf(_);
    if (f !== -1 && (t === document || t === window)) {
      e.__root = t;
      return;
    }
    var m = n.indexOf(t);
    if (m === -1) return;
    f <= m && (i = f);
  }
  if (a = n[i] || e.target, a !== t) {
    z(e, "currentTarget", { configurable: true, get() {
      return a || r;
    } });
    var M = j, v = N;
    A(null), S(null);
    try {
      for (var s, l = []; a !== null; ) {
        var y = a.assignedSlot || a.parentNode || a.host || null;
        try {
          var h = a["__" + o];
          if (h != null && (!a.disabled || e.target === a)) if (J(h)) {
            var [U, ...Y] = h;
            U.apply(a, [e, ...Y]);
          } else h.call(a, e);
        } catch (k) {
          s ? l.push(k) : s = k;
        }
        if (e.cancelBubble || y === t || y === null) break;
        a = y;
      }
      if (s) {
        for (let k of l) queueMicrotask(() => {
          throw k;
        });
        throw s;
      }
    } finally {
      e.__root = t, delete e.currentTarget, A(M), S(v);
    }
  }
}
let c;
function me() {
  c = void 0;
}
function Pe(e) {
  let t = null, r = d;
  var o;
  if (d) {
    for (t = u, c === void 0 && (c = w(document.head)); c !== null && (c.nodeType !== 8 || c.data !== W); ) c = R(c);
    c === null ? E(false) : c = L(R(c));
  }
  d || (o = document.head.appendChild(T()));
  try {
    K(() => e(o), Q);
  } finally {
    r && (E(true), c = u, L(t));
  }
}
function B(e) {
  var t = document.createElement("template");
  return t.innerHTML = e.replaceAll("<!>", "<!---->"), t.content;
}
function p(e, t) {
  var r = N;
  r.nodes_start === null && (r.nodes_start = e, r.nodes_end = t);
}
function Re(e, t) {
  var r = (t & Z) !== 0, o = (t & ee) !== 0, n, a = !e.startsWith("<!>");
  return () => {
    if (d) return p(u, null), u;
    n === void 0 && (n = B(a ? e : "<!>" + e), r || (n = w(n)));
    var i = o || X ? document.importNode(n, true) : n.cloneNode(true);
    if (r) {
      var _ = w(i), f = i.lastChild;
      p(_, f);
    } else p(i, i);
    return i;
  };
}
function ye(e, t, r = "svg") {
  var o = !e.startsWith("<!>"), n = `<${r}>${o ? e : "<!>" + e}</${r}>`, a;
  return () => {
    if (d) return p(u, null), u;
    if (!a) {
      var i = B(n), _ = w(i);
      a = w(_);
    }
    var f = a.cloneNode(true);
    return p(f, f), f;
  };
}
function Ie(e, t) {
  return ye(e, t, "svg");
}
function Oe(e = "") {
  if (!d) {
    var t = T(e + "");
    return p(t, t), t;
  }
  var r = u;
  return r.nodeType !== 3 && (r.before(r = T()), L(r)), p(r, r), r;
}
function Ce() {
  if (d) return p(u, null), u;
  var e = document.createDocumentFragment(), t = document.createComment(""), r = T();
  return e.append(t, r), p(t, r), e;
}
function De(e, t) {
  if (d) {
    N.nodes_end = u, F();
    return;
  }
  e !== null && e.before(t);
}
let V = true;
function Ve(e, t) {
  var r = t == null ? "" : typeof t == "object" ? t + "" : t;
  r !== (e.__t ?? (e.__t = e.nodeValue)) && (e.__t = r, e.nodeValue = r + "");
}
function ge(e, t) {
  return G(e, t);
}
function xe(e, t) {
  I(), t.intro = t.intro ?? false;
  const r = t.target, o = d, n = u;
  try {
    for (var a = w(r); a && (a.nodeType !== 8 || a.data !== W); ) a = R(a);
    if (!a) throw P;
    E(true), L(a), F();
    const i = G(e, { ...t, anchor: a });
    if (u === null || u.nodeType !== 8 || u.data !== te) throw re(), P;
    return E(false), i;
  } catch (i) {
    if (i === P) return t.recover === false && ae(), I(), ne(r), E(false), ge(e, t);
    throw i;
  } finally {
    E(o), L(n), me();
  }
}
const g = /* @__PURE__ */ new Map();
function G(e, { target: t, anchor: r, props: o = {}, events: n, context: a, intro: i = true }) {
  I();
  var _ = /* @__PURE__ */ new Set(), f = (v) => {
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
  f(oe(q)), O.add(f);
  var m = void 0, M = ie(() => {
    var v = r ?? t.appendChild(T());
    return se(() => {
      if (a) {
        ue({});
        var s = le;
        s.c = a;
      }
      n && (o.$$events = n), d && p(v, null), V = i, m = e(v, o) || {}, V = true, d && (N.nodes_end = u), a && fe();
    }), () => {
      var _a2;
      for (var s of _) {
        t.removeEventListener(s, b);
        var l = g.get(s);
        --l === 0 ? (document.removeEventListener(s, b), g.delete(s)) : g.set(s, l);
      }
      O.delete(f), v !== r && ((_a2 = v.parentNode) == null ? void 0 : _a2.removeChild(v));
    };
  });
  return C.set(m, M), m;
}
let C = /* @__PURE__ */ new WeakMap();
function He(e, t) {
  const r = C.get(e);
  return r ? (C.delete(e), r(t)) : Promise.resolve();
}
const Ee = "5";
typeof window < "u" && ((_a = window.__svelte ?? (window.__svelte = {})).v ?? (_a.v = /* @__PURE__ */ new Set())).add(Ee);
export {
  De as a,
  he as b,
  Ce as c,
  Me as d,
  Ne as e,
  Re as f,
  Te as g,
  xe as h,
  be as i,
  pe as j,
  V as k,
  Ae as l,
  ge as m,
  Le as n,
  Ie as o,
  Se as p,
  Pe as q,
  ke as r,
  Ve as s,
  Oe as t,
  He as u,
  $ as w
};
