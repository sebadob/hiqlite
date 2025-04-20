import { t as b, a as k, s as i } from "../chunks/B3zjZD7z.js";
import "../chunks/ByraeRS2.js";
import { c as x, u as $, a as l, r as u, b as y, d as j, g as v, f as E, h as q, p as w, i as z, t as A, j as B, k as m, l as g, s as C } from "../chunks/DEAb5m-A.js";
import { s as D, p as d } from "../chunks/X1x_5zTn.js";
function F(a = false) {
  const e = x, t = e.l.u;
  if (!t) return;
  let r = () => E(e.s);
  if (a) {
    let o = 0, s = {};
    const f = q(() => {
      let c = false;
      const p = e.s;
      for (const n in p) p[n] !== s[n] && (s[n] = p[n], c = true);
      return c && o++, o;
    });
    r = () => v(f);
  }
  t.b.length && $(() => {
    h(e, r), u(t.b);
  }), l(() => {
    const o = y(() => t.m.map(j));
    return () => {
      for (const s of o) typeof s == "function" && s();
    };
  }), t.a.length && l(() => {
    h(e, r), u(t.a);
  });
}
function h(a, e) {
  if (a.l.s) for (const t of a.l.s) v(t);
  e();
}
const G = { get error() {
  return d.error;
}, get status() {
  return d.status;
} };
D.updated.check;
const _ = G;
var H = b("<h1> </h1> <p> </p>", 1);
function M(a, e) {
  w(e, false), F();
  var t = H(), r = z(t), o = m(r, true);
  g(r);
  var s = C(r, 2), f = m(s, true);
  g(s), A(() => {
    var _a;
    i(o, _.status), i(f, (_a = _.error) == null ? void 0 : _a.message);
  }), k(a, t), B();
}
export {
  M as component
};
