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
var _s, _r, _e2, _t, _a, _n;
import { m as A, n as U, o as g, q as Y, v as R, w as T, x as P, y as F, z as C, A as $, B as k, C as q, E as K, D as z, H as G, F as H, G as Z, I, J, K as Q, b as D, L as V, S as x, M as O, N as W, O as X, P as j, g as p, Q as ee, R as se, T as M, U as re, V as te, W as ae, h as ne, X as ie, Y as fe, Z as ue, _ as ce, $ as de, a0 as oe, a1 as le, a2 as _e, a3 as he, a4 as ve, a5 as be } from "./BDwp15xD.js";
class pe {
  constructor(s, t = true) {
    __publicField(this, "anchor");
    __privateAdd(this, _s, /* @__PURE__ */ new Map());
    __privateAdd(this, _r, /* @__PURE__ */ new Map());
    __privateAdd(this, _e2, /* @__PURE__ */ new Map());
    __privateAdd(this, _t, true);
    __privateAdd(this, _a, () => {
      var s = A;
      if (__privateGet(this, _s).has(s)) {
        var t = __privateGet(this, _s).get(s), r = __privateGet(this, _r).get(t);
        if (r) U(r);
        else {
          var n = __privateGet(this, _e2).get(t);
          n && (__privateGet(this, _r).set(t, n.effect), __privateGet(this, _e2).delete(t), n.fragment.lastChild.remove(), this.anchor.before(n.fragment), r = n.effect);
        }
        for (const [a, f] of __privateGet(this, _s)) {
          if (__privateGet(this, _s).delete(a), a === s) break;
          const i = __privateGet(this, _e2).get(f);
          i && (g(i.effect), __privateGet(this, _e2).delete(f));
        }
        for (const [a, f] of __privateGet(this, _r)) {
          if (a === t) continue;
          const i = () => {
            if (Array.from(__privateGet(this, _s).values()).includes(a)) {
              var d = document.createDocumentFragment();
              C(f, d), d.append(R()), __privateGet(this, _e2).set(a, { effect: f, fragment: d });
            } else g(f);
            __privateGet(this, _r).delete(a);
          };
          __privateGet(this, _t) || !r ? Y(f, i, false) : i();
        }
      }
    });
    __privateAdd(this, _n, (s) => {
      __privateGet(this, _s).delete(s);
      const t = Array.from(__privateGet(this, _s).values());
      for (const [r, n] of __privateGet(this, _e2)) t.includes(r) || (g(n.effect), __privateGet(this, _e2).delete(r));
    });
    this.anchor = s, __privateSet(this, _t, t);
  }
  ensure(s, t) {
    var r = A, n = $();
    if (t && !__privateGet(this, _r).has(s) && !__privateGet(this, _e2).has(s)) if (n) {
      var a = document.createDocumentFragment(), f = R();
      a.append(f), __privateGet(this, _e2).set(s, { effect: T(() => t(f)), fragment: a });
    } else __privateGet(this, _r).set(s, T(() => t(this.anchor)));
    if (__privateGet(this, _s).set(r, s), n) {
      for (const [i, u] of __privateGet(this, _r)) i === s ? r.skipped_effects.delete(u) : r.skipped_effects.add(u);
      for (const [i, u] of __privateGet(this, _e2)) i === s ? r.skipped_effects.delete(u.effect) : r.skipped_effects.add(u.effect);
      r.oncommit(__privateGet(this, _a)), r.ondiscard(__privateGet(this, _n));
    } else P && (this.anchor = F), __privateGet(this, _a).call(this);
  }
}
_s = new WeakMap();
_r = new WeakMap();
_e2 = new WeakMap();
_t = new WeakMap();
_a = new WeakMap();
_n = new WeakMap();
function me(e, s, t = false) {
  P && q();
  var r = new pe(e), n = t ? K : 0;
  function a(f, i) {
    if (P) {
      const d = z(e) === G;
      if (f === d) {
        var u = H();
        Z(u), r.anchor = u, I(false), r.ensure(f, i), I(true);
        return;
      }
    }
    r.ensure(f, i);
  }
  k(() => {
    var f = false;
    s((i, u = true) => {
      f = true, a(u, i);
    }), f || a(false, null);
  }, n);
}
function w(e, s) {
  return e === s || (e == null ? void 0 : e[x]) === s;
}
function Ee(e = {}, s, t, r) {
  return J(() => {
    var n, a;
    return Q(() => {
      n = a, a = (r == null ? void 0 : r()) || [], D(() => {
        e !== t(...a) && (s(e, ...a), n && w(t(...n), e) && s(null, ...n));
      });
    }), () => {
      V(() => {
        a && w(t(...a), e) && s(null, ...a);
      });
    };
  }), e;
}
let b = false, m = Symbol();
function ye(e, s, t) {
  const r = t[s] ?? (t[s] = { store: null, source: W(void 0), unsubscribe: O });
  if (r.store !== e && !(m in t)) if (r.unsubscribe(), r.store = e ?? null, e == null) r.source.v = void 0, r.unsubscribe = O;
  else {
    var n = true;
    r.unsubscribe = X(e, (a) => {
      n ? r.source.v = a : M(r.source, a);
    }), n = false;
  }
  return e && m in t ? j(e) : p(r.source);
}
function Ae() {
  const e = {};
  function s() {
    ee(() => {
      for (var t in e) e[t].unsubscribe();
      se(e, m, { enumerable: false, value: true });
    });
  }
  return [e, s];
}
function Se(e) {
  var s = b;
  try {
    return b = false, [e(), b];
  } finally {
    b = s;
  }
}
const ge = { get(e, s) {
  if (!e.exclude.includes(s)) return e.props[s];
}, set(e, s) {
  return false;
}, getOwnPropertyDescriptor(e, s) {
  if (!e.exclude.includes(s) && s in e.props) return { enumerable: true, configurable: true, value: e.props[s] };
}, has(e, s) {
  return e.exclude.includes(s) ? false : s in e.props;
}, ownKeys(e) {
  return Reflect.ownKeys(e.props).filter((s) => !e.exclude.includes(s));
} };
function Re(e, s, t) {
  return new Proxy({ props: e, exclude: s }, ge);
}
function Te(e, s, t, r) {
  var _a2;
  var n = !le || (t & _e) !== 0, a = (t & oe) !== 0, f = (t & ve) !== 0, i = r, u = true, d = () => (u && (u = false, i = f ? D(r) : r), i), l;
  if (a) {
    var N = x in e || be in e;
    l = ((_a2 = re(e, s)) == null ? void 0 : _a2.set) ?? (N && s in e ? (c) => e[s] = c : void 0);
  }
  var _, E = false;
  a ? [_, E] = Se(() => e[s]) : _ = e[s], _ === void 0 && r !== void 0 && (_ = d(), l && (n && te(), l(_)));
  var o;
  if (n ? o = () => {
    var c = e[s];
    return c === void 0 ? d() : (u = true, c);
  } : o = () => {
    var c = e[s];
    return c !== void 0 && (i = void 0), c === void 0 ? i : c;
  }, n && (t & ae) === 0) return o;
  if (l) {
    var L = e.$$legacy;
    return (function(c, v) {
      return arguments.length > 0 ? ((!n || !v || L || E) && l(v ? o() : c), c) : o();
    });
  }
  var S = false, h = ((t & he) !== 0 ? ne : ie)(() => (S = false, o()));
  a && p(h);
  var B = ce;
  return (function(c, v) {
    if (arguments.length > 0) {
      const y = v ? p(h) : n && a ? fe(c) : c;
      return M(h, y), S = true, i !== void 0 && (i = y), c;
    }
    return ue && S || (B.f & de) !== 0 ? h.v : p(h);
  });
}
export {
  pe as B,
  ye as a,
  Ee as b,
  me as i,
  Te as p,
  Re as r,
  Ae as s
};
