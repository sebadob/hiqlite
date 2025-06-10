import { m as G, n as h, o as H, E as K, q as Z, H as z, v as V, w as J, x as Q, y as L, z as m, A as q, B as w, U as W, C as X, D as j, F as x, b as O, G as k, S as M, I as U, J as C, K as ee, L as re, g as b, M as se, N as ne, O as $, P as ae, Q as ue, R as ie, h as Y, T as te, V as fe, W as le, X as ce, Y as _e, Z as oe, _ as ve, $ as de, a0 as be, a1 as Ie } from "./CxznHt52.js";
function Ee(e, r, [a, s] = [0, 0]) {
  h && a === 0 && H();
  var i = e, n = null, t = null, d = W, P = a > 0 ? K : 0, l = false;
  const R = (f, c = true) => {
    l = true, v(c, f);
  }, v = (f, c) => {
    if (d === (d = f)) return;
    let I = false;
    if (h && s !== -1) {
      if (a === 0) {
        const _ = Z(i);
        _ === z ? s = 0 : _ === V ? s = 1 / 0 : (s = parseInt(_.substring(1)), s !== s && (s = d ? 1 / 0 : -1));
      }
      const p = s > a;
      !!d === p && (i = J(), Q(i), L(false), I = true, s = -1);
    }
    d ? (n ? m(n) : c && (n = q(() => c(i))), t && w(t, () => {
      t = null;
    })) : (t ? m(t) : c && (t = q(() => c(i, [a + 1, s]))), n && w(n, () => {
      n = null;
    })), I && L(true);
  };
  G(() => {
    l = false, r(R), l || v(null, null);
  }, P), h && (i = X);
}
function y(e, r) {
  return e === r || (e == null ? void 0 : e[M]) === r;
}
function Re(e = {}, r, a, s) {
  return j(() => {
    var i, n;
    return x(() => {
      i = n, n = (s == null ? void 0 : s()) || [], O(() => {
        e !== a(...n) && (r(e, ...n), i && y(a(...i), e) && r(null, ...i));
      });
    }), () => {
      k(() => {
        n && y(a(...n), e) && r(null, ...n);
      });
    };
  }), e;
}
let E = false, N = Symbol();
function Te(e, r, a) {
  const s = a[r] ?? (a[r] = { store: null, source: C(void 0), unsubscribe: U });
  if (s.store !== e && !(N in a)) if (s.unsubscribe(), s.store = e ?? null, e == null) s.source.v = void 0, s.unsubscribe = U;
  else {
    var i = true;
    s.unsubscribe = ee(e, (n) => {
      i ? s.source.v = n : $(s.source, n);
    }), i = false;
  }
  return e && N in a ? re(e) : b(s.source);
}
function Ae() {
  const e = {};
  function r() {
    se(() => {
      for (var a in e) e[a].unsubscribe();
      ne(e, N, { enumerable: false, value: true });
    });
  }
  return [e, r];
}
function Se(e) {
  var r = E;
  try {
    return E = false, [e(), E];
  } finally {
    E = r;
  }
}
const Pe = { get(e, r) {
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
function ge(e, r, a) {
  return new Proxy({ props: e, exclude: r }, Pe);
}
function B(e) {
  var _a;
  return ((_a = e.ctx) == null ? void 0 : _a.d) ?? false;
}
function he(e, r, a, s) {
  var _a;
  var i = (a & be) !== 0, n = !ve || (a & de) !== 0, t = (a & _e) !== 0, d = (a & Ie) !== 0, P = false, l;
  t ? [l, P] = Se(() => e[r]) : l = e[r];
  var R = M in e || oe in e, v = t && (((_a = ae(e, r)) == null ? void 0 : _a.set) ?? (R && r in e && ((u) => e[r] = u))) || void 0, f = s, c = true, I = false, p = () => (I = true, c && (c = false, d ? f = O(s) : f = s), f);
  l === void 0 && s !== void 0 && (v && n && ue(), l = p(), v && v(l));
  var _;
  if (n) _ = () => {
    var u = e[r];
    return u === void 0 ? p() : (c = true, I = false, u);
  };
  else {
    var D = (i ? Y : te)(() => e[r]);
    D.f |= ie, _ = () => {
      var u = b(D);
      return u !== void 0 && (f = void 0), u === void 0 ? f : u;
    };
  }
  if ((a & fe) === 0 && n) return _;
  if (v) {
    var F = e.$$legacy;
    return function(u, S) {
      return arguments.length > 0 ? ((!n || !S || F || P) && v(S ? _() : u), u) : _();
    };
  }
  var T = false, A = C(l), o = Y(() => {
    var u = _(), S = b(A);
    return T ? (T = false, S) : A.v = u;
  });
  return t && b(o), i || (o.equals = le), function(u, S) {
    if (arguments.length > 0) {
      const g = S ? b(o) : n && t ? ce(u) : u;
      if (!o.equals(g)) {
        if (T = true, $(A, g), I && f !== void 0 && (f = g), B(o)) return u;
        O(() => b(o));
      }
      return u;
    }
    return B(o) ? o.v : b(o);
  };
}
export {
  Te as a,
  Re as b,
  Ee as i,
  he as p,
  ge as r,
  Ae as s
};
