var __defProp = Object.defineProperty;
var __typeError = (msg) => {
  throw TypeError(msg);
};
var __defNormalProp = (obj, key, value) => key in obj ? __defProp(obj, key, { enumerable: true, configurable: true, writable: true, value }) : obj[key] = value;
var __publicField = (obj, key, value) => __defNormalProp(obj, typeof key !== "symbol" ? key + "" : key, value);
var __accessCheck = (obj, member, msg) => member.has(obj) || __typeError("Cannot " + msg);
var __privateGet = (obj, member, getter) => (__accessCheck(obj, member, "read from private field"), getter ? getter.call(obj) : member.get(obj));
var __privateAdd = (obj, member, value) => member.has(obj) ? __typeError("Cannot add the same private member more than once") : member instanceof WeakSet ? member.add(obj) : member.set(obj, value);
var __privateSet = (obj, member, value, setter) => (__accessCheck(obj, member, "write to private field"), setter ? setter.call(obj, value) : member.set(obj, value), value);
var __privateMethod = (obj, member, method) => (__accessCheck(obj, member, "access private method"), method);
var _r, _t, _v, _s, _c, _i, _n, _e2, _a, _o, _u, _h, _l, _d, _f, _y, _Ie_instances, E_fn, b_fn, g_fn, __fn, p_fn, m_fn, _a2;
import { ab as J, g as $, ac as G, K as Z, b as ee, ad as H, L as V, y as f, x as h, _ as v, B as te, C as U, ae as B, H as re, w as p, af as A, q as O, v as T, ag as L, ah as E, ai as W, aj as ne, ak as C, c as z, z as se, al as ie, o as F, G as N, am as ae, F as oe, an as j, ao as ue, E as le, ap as fe, aq as ce, ar as he, R as de, as as _e, Q as pe, at as R, au as ve, av as ge, aw as me, ax as ye, ay as Ee, az as I, aA as be, aB as we, aC as M, I as S, aD as Te, aE as Ne, aF as Re, aG as ke, p as Ae, aH as Se, aI as Le, j as De } from "./BDwp15xD.js";
function Oe(t) {
  let e = 0, r = G(0), i;
  return () => {
    J() && ($(r), Z(() => (e === 0 && (i = ee(() => t(() => H(r)))), e += 1, () => {
      V(() => {
        e -= 1, e === 0 && (i == null ? void 0 : i(), i = void 0, H(r));
      });
    })));
  };
}
var Fe = le | fe | ce;
function Ce(t, e, r) {
  new Ie(t, e, r);
}
class Ie {
  constructor(e, r, i) {
    __privateAdd(this, _Ie_instances);
    __publicField(this, "parent");
    __privateAdd(this, _r, false);
    __privateAdd(this, _t);
    __privateAdd(this, _v, h ? f : null);
    __privateAdd(this, _s);
    __privateAdd(this, _c);
    __privateAdd(this, _i);
    __privateAdd(this, _n, null);
    __privateAdd(this, _e2, null);
    __privateAdd(this, _a, null);
    __privateAdd(this, _o, null);
    __privateAdd(this, _u, null);
    __privateAdd(this, _h, 0);
    __privateAdd(this, _l, 0);
    __privateAdd(this, _d, false);
    __privateAdd(this, _f, null);
    __privateAdd(this, _y, Oe(() => (__privateSet(this, _f, G(__privateGet(this, _h))), () => {
      __privateSet(this, _f, null);
    })));
    __privateSet(this, _t, e), __privateSet(this, _s, r), __privateSet(this, _c, i), this.parent = v.b, __privateSet(this, _r, !!__privateGet(this, _s).pending), __privateSet(this, _i, te(() => {
      if (v.b = this, h) {
        const n = __privateGet(this, _v);
        U(), n.nodeType === B && n.data === re ? __privateMethod(this, _Ie_instances, b_fn).call(this) : __privateMethod(this, _Ie_instances, E_fn).call(this);
      } else {
        var s = __privateMethod(this, _Ie_instances, g_fn).call(this);
        try {
          __privateSet(this, _n, p(() => i(s)));
        } catch (n) {
          this.error(n);
        }
        __privateGet(this, _l) > 0 ? __privateMethod(this, _Ie_instances, p_fn).call(this) : __privateSet(this, _r, false);
      }
      return () => {
        var _a3;
        (_a3 = __privateGet(this, _u)) == null ? void 0 : _a3.remove();
      };
    }, Fe)), h && __privateSet(this, _t, f);
  }
  is_pending() {
    return __privateGet(this, _r) || !!this.parent && this.parent.is_pending();
  }
  has_pending_snippet() {
    return !!__privateGet(this, _s).pending;
  }
  update_pending_count(e) {
    __privateMethod(this, _Ie_instances, m_fn).call(this, e), __privateSet(this, _h, __privateGet(this, _h) + e), __privateGet(this, _f) && ie(__privateGet(this, _f), __privateGet(this, _h));
  }
  get_effect_pending() {
    return __privateGet(this, _y).call(this), $(__privateGet(this, _f));
  }
  error(e) {
    var r = __privateGet(this, _s).onerror;
    let i = __privateGet(this, _s).failed;
    if (__privateGet(this, _d) || !r && !i) throw e;
    __privateGet(this, _n) && (F(__privateGet(this, _n)), __privateSet(this, _n, null)), __privateGet(this, _e2) && (F(__privateGet(this, _e2)), __privateSet(this, _e2, null)), __privateGet(this, _a) && (F(__privateGet(this, _a)), __privateSet(this, _a, null)), h && (N(__privateGet(this, _v)), ae(), N(oe()));
    var s = false, n = false;
    const a = () => {
      if (s) {
        he();
        return;
      }
      s = true, n && ue(), A.ensure(), __privateSet(this, _h, 0), __privateGet(this, _a) !== null && O(__privateGet(this, _a), () => {
        __privateSet(this, _a, null);
      }), __privateSet(this, _r, this.has_pending_snippet()), __privateSet(this, _n, __privateMethod(this, _Ie_instances, __fn).call(this, () => (__privateSet(this, _d, false), p(() => __privateGet(this, _c).call(this, __privateGet(this, _t)))))), __privateGet(this, _l) > 0 ? __privateMethod(this, _Ie_instances, p_fn).call(this) : __privateSet(this, _r, false);
    };
    var c = C;
    try {
      E(null), n = true, r == null ? void 0 : r(e, a), n = false;
    } catch (o) {
      j(o, __privateGet(this, _i) && __privateGet(this, _i).parent);
    } finally {
      E(c);
    }
    i && V(() => {
      __privateSet(this, _a, __privateMethod(this, _Ie_instances, __fn).call(this, () => {
        A.ensure(), __privateSet(this, _d, true);
        try {
          return p(() => {
            i(__privateGet(this, _t), () => e, () => a);
          });
        } catch (o) {
          return j(o, __privateGet(this, _i).parent), null;
        } finally {
          __privateSet(this, _d, false);
        }
      }));
    });
  }
}
_r = new WeakMap();
_t = new WeakMap();
_v = new WeakMap();
_s = new WeakMap();
_c = new WeakMap();
_i = new WeakMap();
_n = new WeakMap();
_e2 = new WeakMap();
_a = new WeakMap();
_o = new WeakMap();
_u = new WeakMap();
_h = new WeakMap();
_l = new WeakMap();
_d = new WeakMap();
_f = new WeakMap();
_y = new WeakMap();
_Ie_instances = new WeakSet();
E_fn = function() {
  try {
    __privateSet(this, _n, p(() => __privateGet(this, _c).call(this, __privateGet(this, _t))));
  } catch (e) {
    this.error(e);
  }
  __privateSet(this, _r, false);
};
b_fn = function() {
  const e = __privateGet(this, _s).pending;
  e && (__privateSet(this, _e2, p(() => e(__privateGet(this, _t)))), A.enqueue(() => {
    var r = __privateMethod(this, _Ie_instances, g_fn).call(this);
    __privateSet(this, _n, __privateMethod(this, _Ie_instances, __fn).call(this, () => (A.ensure(), p(() => __privateGet(this, _c).call(this, r))))), __privateGet(this, _l) > 0 ? __privateMethod(this, _Ie_instances, p_fn).call(this) : (O(__privateGet(this, _e2), () => {
      __privateSet(this, _e2, null);
    }), __privateSet(this, _r, false));
  }));
};
g_fn = function() {
  var e = __privateGet(this, _t);
  return __privateGet(this, _r) && (__privateSet(this, _u, T()), __privateGet(this, _t).before(__privateGet(this, _u)), e = __privateGet(this, _u)), e;
};
__fn = function(e) {
  var r = v, i = C, s = z;
  L(__privateGet(this, _i)), E(__privateGet(this, _i)), W(__privateGet(this, _i).ctx);
  try {
    return e();
  } catch (n) {
    return ne(n), null;
  } finally {
    L(r), E(i), W(s);
  }
};
p_fn = function() {
  const e = __privateGet(this, _s).pending;
  __privateGet(this, _n) !== null && (__privateSet(this, _o, document.createDocumentFragment()), __privateGet(this, _o).append(__privateGet(this, _u)), se(__privateGet(this, _n), __privateGet(this, _o))), __privateGet(this, _e2) === null && __privateSet(this, _e2, p(() => e(__privateGet(this, _t))));
};
m_fn = function(e) {
  var _a3;
  if (!this.has_pending_snippet()) {
    this.parent && __privateMethod(_a3 = this.parent, _Ie_instances, m_fn).call(_a3, e);
    return;
  }
  __privateSet(this, _l, __privateGet(this, _l) + e), __privateGet(this, _l) === 0 && (__privateSet(this, _r, false), __privateGet(this, _e2) && O(__privateGet(this, _e2), () => {
    __privateSet(this, _e2, null);
  }), __privateGet(this, _o) && (__privateGet(this, _t).before(__privateGet(this, _o)), __privateSet(this, _o, null)));
};
function Ye(t) {
  return t.endsWith("capture") && t !== "gotpointercapture" && t !== "lostpointercapture";
}
const Me = ["beforeinput", "click", "change", "dblclick", "contextmenu", "focusin", "focusout", "input", "keydown", "keyup", "mousedown", "mousemove", "mouseout", "mouseover", "mouseup", "pointerdown", "pointermove", "pointerout", "pointerover", "pointerup", "touchend", "touchmove", "touchstart"];
function $e(t) {
  return Me.includes(t);
}
const xe = { formnovalidate: "formNoValidate", ismap: "isMap", nomodule: "noModule", playsinline: "playsInline", readonly: "readOnly", defaultvalue: "defaultValue", defaultchecked: "defaultChecked", srcobject: "srcObject", novalidate: "noValidate", allowfullscreen: "allowFullscreen", disablepictureinpicture: "disablePictureInPicture", disableremoteplayback: "disableRemotePlayback" };
function Ge(t) {
  return t = t.toLowerCase(), xe[t] ?? t;
}
const Pe = ["touchstart", "touchmove"];
function Ve(t) {
  return Pe.includes(t);
}
const K = /* @__PURE__ */ new Set(), x = /* @__PURE__ */ new Set();
function Be(t, e, r, i = {}) {
  function s(n) {
    if (i.capture || w.call(e, n), !n.cancelBubble) return _e(() => r == null ? void 0 : r.call(this, n));
  }
  return t.startsWith("pointer") || t.startsWith("touch") || t === "wheel" ? V(() => {
    e.addEventListener(t, s, i);
  }) : e.addEventListener(t, s, i), s;
}
function Ue(t, e, r, i, s) {
  var n = { capture: i, passive: s }, a = Be(t, e, r, n);
  (e === document.body || e === window || e === document || e instanceof HTMLMediaElement) && pe(() => {
    e.removeEventListener(t, a, n);
  });
}
function ze(t) {
  for (var e = 0; e < t.length; e++) K.add(t[e]);
  for (var r of x) r(t);
}
let q = null;
function w(t) {
  var _a3;
  var e = this, r = e.ownerDocument, i = t.type, s = ((_a3 = t.composedPath) == null ? void 0 : _a3.call(t)) || [], n = s[0] || t.target;
  q = t;
  var a = 0, c = q === t && t.__root;
  if (c) {
    var o = s.indexOf(c);
    if (o !== -1 && (e === document || e === window)) {
      t.__root = e;
      return;
    }
    var g = s.indexOf(e);
    if (g === -1) return;
    o <= g && (a = o);
  }
  if (n = s[a] || t.target, n !== e) {
    de(t, "currentTarget", { configurable: true, get() {
      return n || r;
    } });
    var D = C, _ = v;
    E(null), L(null);
    try {
      for (var u, l = []; n !== null; ) {
        var m = n.assignedSlot || n.parentNode || n.host || null;
        try {
          var b = n["__" + i];
          b != null && (!n.disabled || t.target === n) && b.call(n, t);
        } catch (k) {
          u ? l.push(k) : u = k;
        }
        if (t.cancelBubble || m === e || m === null) break;
        n = m;
      }
      if (u) {
        for (let k of l) queueMicrotask(() => {
          throw k;
        });
        throw u;
      }
    } finally {
      t.__root = e, delete t.currentTarget, E(D), L(_);
    }
  }
}
function Q(t) {
  var e = document.createElement("template");
  return e.innerHTML = t.replaceAll("<!>", "<!---->"), e.content;
}
function d(t, e) {
  var r = v;
  r.nodes_start === null && (r.nodes_start = t, r.nodes_end = e);
}
function Ke(t, e) {
  var r = (e & ge) !== 0, i = (e & me) !== 0, s, n = !t.startsWith("<!>");
  return () => {
    if (h) return d(f, null), f;
    s === void 0 && (s = Q(n ? t : "<!>" + t), r || (s = R(s)));
    var a = i || ve ? document.importNode(s, true) : s.cloneNode(true);
    if (r) {
      var c = R(a), o = a.lastChild;
      d(c, o);
    } else d(a, a);
    return a;
  };
}
function He(t, e, r = "svg") {
  var i = !t.startsWith("<!>"), s = `<${r}>${i ? t : "<!>" + t}</${r}>`, n;
  return () => {
    if (h) return d(f, null), f;
    if (!n) {
      var a = Q(s), c = R(a);
      n = R(c);
    }
    var o = n.cloneNode(true);
    return d(o, o), o;
  };
}
function Qe(t, e) {
  return He(t, e, "svg");
}
function Xe(t = "") {
  if (!h) {
    var e = T(t + "");
    return d(e, e), e;
  }
  var r = f;
  return r.nodeType !== Ee && (r.before(r = T()), N(r)), d(r, r), r;
}
function Je() {
  if (h) return d(f, null), f;
  var t = document.createDocumentFragment(), e = document.createComment(""), r = T();
  return t.append(e, r), d(e, r), t;
}
function Ze(t, e) {
  if (h) {
    var r = v;
    ((r.f & ye) === 0 || r.nodes_end === null) && (r.nodes_end = f), U();
    return;
  }
  t !== null && t.before(e);
}
let Y = true;
function et(t, e) {
  var r = e == null ? "" : typeof e == "object" ? e + "" : e;
  r !== (t.__t ?? (t.__t = t.nodeValue)) && (t.__t = r, t.nodeValue = r + "");
}
function We(t, e) {
  return X(t, e);
}
function tt(t, e) {
  I(), e.intro = e.intro ?? false;
  const r = e.target, i = h, s = f;
  try {
    for (var n = R(r); n && (n.nodeType !== B || n.data !== be); ) n = we(n);
    if (!n) throw M;
    S(true), N(n);
    const a = X(t, { ...e, anchor: n });
    return S(false), a;
  } catch (a) {
    if (a instanceof Error && a.message.split(`
`).some((c) => c.startsWith("https://svelte.dev/e/"))) throw a;
    return a !== M && console.warn("Failed to hydrate: ", a), e.recover === false && Te(), I(), Ne(r), S(false), We(t, e);
  } finally {
    S(i), N(s);
  }
}
const y = /* @__PURE__ */ new Map();
function X(t, { target: e, anchor: r, props: i = {}, events: s, context: n, intro: a = true }) {
  I();
  var c = /* @__PURE__ */ new Set(), o = (_) => {
    for (var u = 0; u < _.length; u++) {
      var l = _[u];
      if (!c.has(l)) {
        c.add(l);
        var m = Ve(l);
        e.addEventListener(l, w, { passive: m });
        var b = y.get(l);
        b === void 0 ? (document.addEventListener(l, w, { passive: m }), y.set(l, 1)) : y.set(l, b + 1);
      }
    }
  };
  o(Re(K)), x.add(o);
  var g = void 0, D = ke(() => {
    var _ = r ?? e.appendChild(T());
    return Ce(_, { pending: () => {
    } }, (u) => {
      if (n) {
        Ae({});
        var l = z;
        l.c = n;
      }
      if (s && (i.$$events = s), h && d(u, null), Y = a, g = t(u, i) || {}, Y = true, h && (v.nodes_end = f, f === null || f.nodeType !== B || f.data !== Se)) throw Le(), M;
      n && De();
    }), () => {
      var _a3;
      for (var u of c) {
        e.removeEventListener(u, w);
        var l = y.get(u);
        --l === 0 ? (document.removeEventListener(u, w), y.delete(u)) : y.set(u, l);
      }
      x.delete(o), _ !== r && ((_a3 = _.parentNode) == null ? void 0 : _a3.removeChild(_));
    };
  });
  return P.set(g, D), g;
}
let P = /* @__PURE__ */ new WeakMap();
function rt(t, e) {
  const r = P.get(t);
  return r ? (P.delete(t), r(e)) : Promise.resolve();
}
const je = "5";
typeof window < "u" && ((_a2 = window.__svelte ?? (window.__svelte = {})).v ?? (_a2.v = /* @__PURE__ */ new Set())).add(je);
export {
  Ze as a,
  Be as b,
  Je as c,
  ze as d,
  $e as e,
  Ke as f,
  Y as g,
  tt as h,
  Ye as i,
  Qe as j,
  Ue as k,
  We as m,
  Ge as n,
  et as s,
  Xe as t,
  rt as u
};
