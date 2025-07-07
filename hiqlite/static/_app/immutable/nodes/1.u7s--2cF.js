import "../chunks/NZTpNUN0.js";
import "../chunks/BXesWDf4.js";
import { c as b, u as k, a as i, r as l, b as x, d as $, g as v, f as y, h as j, p as E, i as q, j as w, t as z, k as A, l as B, m as u, n as m, s as C, o as g } from "../chunks/CYo-iuqb.js";
import { s as D, p as h } from "../chunks/DAnHfLS2.js";
function F(a = false) {
  const e = b, t = e.l.u;
  if (!t) return;
  let r = () => y(e.s);
  if (a) {
    let o = 0, s = {};
    const f = j(() => {
      let p = false;
      const c = e.s;
      for (const n in c) c[n] !== s[n] && (s[n] = c[n], p = true);
      return p && o++, o;
    });
    r = () => v(f);
  }
  t.b.length && k(() => {
    _(e, r), l(t.b);
  }), i(() => {
    const o = x(() => t.m.map($));
    return () => {
      for (const s of o) typeof s == "function" && s();
    };
  }), t.a.length && i(() => {
    _(e, r), l(t.a);
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
var H = q("<h1> </h1> <p> </p>", 1);
function M(a, e) {
  E(e, false), F();
  var t = H(), r = w(t), o = u(r, true);
  m(r);
  var s = C(r, 2), f = u(s, true);
  m(s), z(() => {
    var _a;
    g(o, d.status), g(f, (_a = d.error) == null ? void 0 : _a.message);
  }), A(a, t), B();
}
export {
  M as component
};
