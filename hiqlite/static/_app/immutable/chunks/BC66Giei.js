import { m as G, n as O, o as H, E as K, H as Z, q as z, v as V, w as J, x as L, y as m, z as q, A as w, U as Q, B as W, C as X, D as j, b as h, F as x, S as M, G as U, I as C, J as k, K as ee, g as b, L as re, M as se, N as $, O as ne, P as ae, Q as ue, h as Y, R as ie, T as fe, V as te, W as le, X as ce, Y as _e, Z as ve, _ as oe, $ as de, a0 as be } from "./BydrjbDF.js";
function pe(e, r, [n, s] = [0, 0]) {
  O && n === 0 && H();
  var i = e, a = null, f = null, d = Q, P = n > 0 ? K : 0, l = false;
  const R = (t, c = true) => {
    l = true, o(c, t);
  }, o = (t, c) => {
    if (d === (d = t)) return;
    let I = false;
    if (O && s !== -1) {
      if (n === 0) {
        const _ = i.data;
        _ === Z ? s = 0 : _ === z ? s = 1 / 0 : (s = parseInt(_.substring(1)), s !== s && (s = d ? 1 / 0 : -1));
      }
      const p = s > n;
      !!d === p && (i = V(), J(i), L(false), I = true, s = -1);
    }
    d ? (a ? m(a) : c && (a = q(() => c(i))), f && w(f, () => {
      f = null;
    })) : (f ? m(f) : c && (f = q(() => c(i, [n + 1, s]))), a && w(a, () => {
      a = null;
    })), I && L(true);
  };
  G(() => {
    l = false, r(R), l || o(null, null);
  }, P), O && (i = W);
}
function y(e, r) {
  return e === r || (e == null ? void 0 : e[M]) === r;
}
function Ee(e = {}, r, n, s) {
  return X(() => {
    var i, a;
    return j(() => {
      i = a, a = (s == null ? void 0 : s()) || [], h(() => {
        e !== n(...a) && (r(e, ...a), i && y(n(...i), e) && r(null, ...i));
      });
    }), () => {
      x(() => {
        a && y(n(...a), e) && r(null, ...a);
      });
    };
  }), e;
}
let E = false, N = Symbol();
function Re(e, r, n) {
  const s = n[r] ?? (n[r] = { store: null, source: C(void 0), unsubscribe: U });
  if (s.store !== e && !(N in n)) if (s.unsubscribe(), s.store = e ?? null, e == null) s.source.v = void 0, s.unsubscribe = U;
  else {
    var i = true;
    s.unsubscribe = k(e, (a) => {
      i ? s.source.v = a : $(s.source, a);
    }), i = false;
  }
  return e && N in n ? ee(e) : b(s.source);
}
function Te() {
  const e = {};
  function r() {
    re(() => {
      for (var n in e) e[n].unsubscribe();
      se(e, N, { enumerable: false, value: true });
    });
  }
  return [e, r];
}
function Ie(e) {
  var r = E;
  try {
    return E = false, [e(), E];
  } finally {
    E = r;
  }
}
const Se = { get(e, r) {
  if (!e.exclude.includes(r)) return e.props[r];
}, set(e, r) {
  return false;
}, getOwnPropertyDescriptor(e, r) {
  if (!e.exclude.includes(r) && r in e.props) return { enumerable: true, configurable: true, value: e.props[r] };
}, has(e, r) {
  return e.exclude.includes(r) ? false : r in e.props;
}, ownKeys(e) {
  return Reflect.ownKeys(e.props).filter((r) => !e.exclude.includes(r));
} };
function Ae(e, r, n) {
  return new Proxy({ props: e, exclude: r }, Se);
}
function B(e) {
  var _a;
  return ((_a = e.ctx) == null ? void 0 : _a.d) ?? false;
}
function ge(e, r, n, s) {
  var _a;
  var i = (n & de) !== 0, a = !ve || (n & oe) !== 0, f = (n & ce) !== 0, d = (n & be) !== 0, P = false, l;
  f ? [l, P] = Ie(() => e[r]) : l = e[r];
  var R = M in e || _e in e, o = f && (((_a = ne(e, r)) == null ? void 0 : _a.set) ?? (R && r in e && ((u) => e[r] = u))) || void 0, t = s, c = true, I = false, p = () => (I = true, c && (c = false, d ? t = h(s) : t = s), t);
  l === void 0 && s !== void 0 && (o && a && ae(), l = p(), o && o(l));
  var _;
  if (a) _ = () => {
    var u = e[r];
    return u === void 0 ? p() : (c = true, I = false, u);
  };
  else {
    var D = (i ? Y : ie)(() => e[r]);
    D.f |= ue, _ = () => {
      var u = b(D);
      return u !== void 0 && (t = void 0), u === void 0 ? t : u;
    };
  }
  if ((n & fe) === 0) return _;
  if (o) {
    var F = e.$$legacy;
    return function(u, S) {
      return arguments.length > 0 ? ((!a || !S || F || P) && o(S ? _() : u), u) : _();
    };
  }
  var T = false, A = C(l), v = Y(() => {
    var u = _(), S = b(A);
    return T ? (T = false, S) : A.v = u;
  });
  return f && b(v), i || (v.equals = te), function(u, S) {
    if (arguments.length > 0) {
      const g = S ? b(v) : a && f ? le(u) : u;
      if (!v.equals(g)) {
        if (T = true, $(A, g), I && t !== void 0 && (t = g), B(v)) return u;
        h(() => b(v));
      }
      return u;
    }
    return B(v) ? v.v : b(v);
  };
}
export {
  Re as a,
  Ee as b,
  pe as i,
  ge as p,
  Ae as r,
  Te as s
};
