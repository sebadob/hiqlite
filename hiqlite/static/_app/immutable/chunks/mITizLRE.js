import { q as m, v as R, w as y, E as B, x as M, H as q, y as $, z as C, A as F, B as p, C as N, D as O, F as h, U as H, G as K, I as Z, J as z, b as w, K as G, S as U, L as D, M as J, N as Q, O as V, g as T, P as W, Q as X, R as Y, T as j, V as x, W as k, h as ee, X as re, Y as se, Z as ne, _ as ae, $ as ie, a0 as ue, a1 as te, a2 as fe } from "./CYo-iuqb.js";
function de(e, r, [n, s] = [0, 0]) {
  R && n === 0 && y();
  var i = e, a = null, o = null, t = H, b = n > 0 ? B : 0, v = false;
  const d = (l, c = true) => {
    v = true, I(c, l);
  }, I = (l, c) => {
    if (t === (t = l)) return;
    let _ = false;
    if (R && s !== -1) {
      if (n === 0) {
        const f = M(i);
        f === q ? s = 0 : f === $ ? s = 1 / 0 : (s = parseInt(f.substring(1)), s !== s && (s = t ? 1 / 0 : -1));
      }
      const A = s > n;
      !!t === A && (i = C(), F(i), p(false), _ = true, s = -1);
    }
    t ? (a ? N(a) : c && (a = O(() => c(i))), o && h(o, () => {
      o = null;
    })) : (o ? N(o) : c && (o = O(() => c(i, [n + 1, s]))), a && h(a, () => {
      a = null;
    })), _ && p(true);
  };
  m(() => {
    v = false, r(d), v || I(null, null);
  }, b), R && (i = K);
}
function L(e, r) {
  return e === r || (e == null ? void 0 : e[U]) === r;
}
function ve(e = {}, r, n, s) {
  return Z(() => {
    var i, a;
    return z(() => {
      i = a, a = (s == null ? void 0 : s()) || [], w(() => {
        e !== n(...a) && (r(e, ...a), i && L(n(...i), e) && r(null, ...i));
      });
    }), () => {
      G(() => {
        a && L(n(...a), e) && r(null, ...a);
      });
    };
  }), e;
}
let P = false, g = Symbol();
function be(e, r, n) {
  const s = n[r] ?? (n[r] = { store: null, source: J(void 0), unsubscribe: D });
  if (s.store !== e && !(g in n)) if (s.unsubscribe(), s.store = e ?? null, e == null) s.source.v = void 0, s.unsubscribe = D;
  else {
    var i = true;
    s.unsubscribe = Q(e, (a) => {
      i ? s.source.v = a : Y(s.source, a);
    }), i = false;
  }
  return e && g in n ? V(e) : T(s.source);
}
function Ie() {
  const e = {};
  function r() {
    W(() => {
      for (var n in e) e[n].unsubscribe();
      X(e, g, { enumerable: false, value: true });
    });
  }
  return [e, r];
}
function le(e) {
  var r = P;
  try {
    return P = false, [e(), P];
  } finally {
    P = r;
  }
}
const ce = { get(e, r) {
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
function Se(e, r, n) {
  return new Proxy({ props: e, exclude: r }, ce);
}
function _e(e) {
  var _a;
  return ((_a = e.ctx) == null ? void 0 : _a.d) ?? false;
}
function Pe(e, r, n, s) {
  var _a;
  var i = !ae || (n & ie) !== 0, a = (n & ne) !== 0, o = (n & te) !== 0, t = s, b = true, v = () => (b && (b = false, t = o ? w(s) : s), t), d;
  if (a) {
    var I = U in e || fe in e;
    d = ((_a = j(e, r)) == null ? void 0 : _a.set) ?? (I && r in e ? (u) => e[r] = u : void 0);
  }
  var l, c = false;
  a ? [l, c] = le(() => e[r]) : l = e[r], l === void 0 && s !== void 0 && (l = v(), d && (i && x(), d(l)));
  var _;
  if (i ? _ = () => {
    var u = e[r];
    return u === void 0 ? v() : (b = true, u);
  } : _ = () => {
    var u = e[r];
    return u !== void 0 && (t = void 0), u === void 0 ? t : u;
  }, i && (n & k) === 0) return _;
  if (d) {
    var A = e.$$legacy;
    return function(u, S) {
      return arguments.length > 0 ? ((!i || !S || A || c) && d(S ? _() : u), u) : _();
    };
  }
  var f = ((n & ue) !== 0 ? ee : re)(_);
  return a && T(f), function(u, S) {
    if (arguments.length > 0) {
      const E = S ? T(f) : i && a ? se(u) : u;
      return Y(f, E), t !== void 0 && (t = E), u;
    }
    return _e(f) ? f.v : T(f);
  };
}
export {
  be as a,
  ve as b,
  de as i,
  Pe as p,
  Se as r,
  Ie as s
};
