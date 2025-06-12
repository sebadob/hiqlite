import { f as b, a as k, s as i } from "../chunks/C_iGe9Tc.js";
import "../chunks/C0SPxqjE.js";
import { c as x, u as $, a as l, r as u, b as y, d as j, g as v, f as E, h as q, p as w, i as z, t as A, j as B, k as m, l as g, s as C } from "../chunks/CxznHt52.js";
import { s as D, p as h } from "../chunks/ttXnxlq3.js";
function F(a = false) {
  const e = x, t = e.l.u;
  if (!t) return;
  let r = () => E(e.s);
  if (a) {
    let o = 0, s = {};
    const f = q(() => {
      let p = false;
      const c = e.s;
      for (const n in c) c[n] !== s[n] && (s[n] = c[n], p = true);
      return p && o++, o;
    });
    r = () => v(f);
  }
  t.b.length && $(() => {
    _(e, r), u(t.b);
  }), l(() => {
    const o = y(() => t.m.map(j));
    return () => {
      for (const s of o) typeof s == "function" && s();
    };
  }), t.a.length && l(() => {
    _(e, r), u(t.a);
  });
}
function _(a, e) {
  if (a.l.s) for (const t of a.l.s) v(t);
  e();
}
const G = { get error() {
  return h.error;
}, get status() {
  return h.status;
} };
D.updated.check;
const d = G;
var H = b("<h1> </h1> <p> </p>", 1);
function M(a, e) {
  w(e, false), F();
  var t = H(), r = z(t), o = m(r, true);
  g(r);
  var s = C(r, 2), f = m(s, true);
  g(s), A(() => {
    var _a;
    i(o, d.status), i(f, (_a = d.error) == null ? void 0 : _a.message);
  }), k(a, t), B();
}
export {
  M as component
};
