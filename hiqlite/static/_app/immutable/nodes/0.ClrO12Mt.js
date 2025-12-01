import { f as p, a as l, k as le, j as ce, d as ue, s as C, t as B, c as he } from "../chunks/CcmLzrf2.js";
import { p as O, a as we, g as r, a8 as A, T, j as Q, i as z, s as m, k as _, l as c, t as L, J as Ae, bb as Ve, am as E, aa as ge, Y as de, a7 as be, bc as Ee, bd as He, be as ze } from "../chunks/3KHOJ3O8.js";
import { i as V, p as S } from "../chunks/BR4MeHRE.js";
import { B as pe, t as Z, c as Re, s as fe, a as k, h as De, r as Be, b as _e, d as Ue, I as Fe, e as Oe, f as ye, A as xe, g as ve, i as ke, j as Qe, k as Se, l as Pe, m as Me, R as Ye, n as Ge, Q as Je, o as Ke, D as Xe } from "../chunks/BTormJyb.js";
import "../chunks/BYtxu643.js";
const Ne = true, Gt = Object.freeze(Object.defineProperty({ __proto__: null, prerender: Ne }, Symbol.toStringTag, { value: "Module" }));
var We = p(`<div class="icon moon svelte-mls84d"><svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor" class="w-6 h-6"><path stroke-linecap="round" stroke-linejoin="round" d="M21.752 15.002A9.718 9.718 0 0118 15.75c-5.385 0-9.75-4.365-9.75-9.75
                        0-1.33.266-2.597.748-3.752A9.753 9.753 0 003 11.25C3 16.635 7.365 21 12.75
                        21a9.753 9.753 0 009.002-5.998z"></path></svg></div>`), Ze = p(`<div class="icon sun svelte-mls84d"><svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor" class="w-6 h-6"><path stroke-linecap="round" stroke-linejoin="round" d="M12 3v2.25m6.364.386l-1.591 1.591M21 12h-2.25m-.386 6.364l-1.591-1.591M12
                        18.75V21m-4.773-4.227l-1.591 1.591M5.25 12H3m4.227-4.773L5.636 5.636M15.75 12a3.75
                        3.75 0 11-7.5 0 3.75 3.75 0 017.5 0z"></path></svg></div>`), $e = p("<!> <!>", 1);
function et(i, e) {
  O(e, true);
  const t = "darkMode", [s, a] = Re({});
  let o = A(void 0);
  we(() => {
    var _a, _b, _c;
    const y = ((_b = (_a = window == null ? void 0 : window.matchMedia) == null ? void 0 : _a.call(window, "(prefers-color-scheme:dark)")) == null ? void 0 : _b.matches) ?? false;
    if (r(o) === void 0) {
      const w = (_c = window == null ? void 0 : window.localStorage) == null ? void 0 : _c.getItem(t);
      if (w) {
        let v = w === "true";
        r(o) === y && localStorage.removeItem(t), T(o, v);
      } else T(o, y, true);
    } else r(o) ? (document.body.classList.remove("light-theme"), document.body.classList.add("dark-theme")) : (document.body.classList.remove("dark-theme"), document.body.classList.add("light-theme"));
    y === r(o) ? localStorage.removeItem(t) : localStorage.setItem(t, r(o).toString());
  });
  function j() {
    T(o, !r(o));
  }
  pe(i, { ariaLabel: "Change color theme", invisible: true, onclick: j, children: (y, w) => {
    var v = $e(), u = z(v);
    {
      var g = (n) => {
        var b = We();
        Z(1, b, () => a, () => ({ key: "dark" })), Z(2, b, () => s, () => ({ key: "light" })), l(n, b);
      };
      V(u, (n) => {
        r(o) === true && n(g);
      });
    }
    var x = m(u, 2);
    {
      var P = (n) => {
        var b = Ze();
        Z(1, b, () => a, () => ({ key: "light" })), Z(2, b, () => s, () => ({ key: "dark" })), l(n, b);
      };
      V(x, (n) => {
        r(o) === false && n(P);
      });
    }
    l(y, v);
  }, $$slots: { default: true } }), Q();
}
var tt = p('<div class="theme-switch svelte-11md1k0"><!></div>');
function at(i) {
  var e = tt(), t = _(e);
  et(t, {}), c(e), l(i, e);
}
var rt = p("<form><!></form>");
function ot(i, e) {
  O(e, true);
  let t = S(e, "method", 3, "POST"), s = S(e, "isError", 15);
  async function a(y) {
    y.preventDefault();
    const w = y.currentTarget;
    if (w.reportValidity()) s(false);
    else {
      s(true);
      return;
    }
    const u = new FormData(w);
    let g = new URLSearchParams();
    if (u.forEach((P, n) => {
      g.append(n, P.toString());
    }), e.onSubmit) {
      e.onSubmit(w, g);
      return;
    }
    const x = await fetch(w.action, { method: w.method, headers: { "Content-type": "application/x-www-form-urlencoded" }, body: g });
    De(x), e.onResponse && (e.onResponse(x), x.ok && w.reset());
  }
  var o = rt(), j = _(o);
  fe(j, () => e.children), c(o), L(() => {
    k(o, "action", e.action), k(o, "method", t());
  }), le("submit", o, a), l(i, o), Q();
}
var it = ce(`<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M8.25 7.5V6.108c0-1.135.845-2.098 1.976-2.192.373-.03.748-.057 1.123-.08M15.75 18H18a2.25 2.25 0
            002.25-2.25V6.108c0-1.135-.845-2.098-1.976-2.192a48.424 48.424 0 00-1.123-.08M15.75 18.75v-1.875a3.375
            3.375 0 00-3.375-3.375h-1.5a1.125 1.125 0 01-1.125-1.125v-1.5A3.375 3.375 0 006.375
            7.5H5.25m11.9-3.664A2.251 2.251 0 0015 2.25h-1.5a2.251 2.251 0 00-2.15 1.586m5.8
            0c.065.21.1.433.1.664v.75h-6V4.5c0-.231.035-.454.1-.664M6.75 7.5H4.875c-.621 0-1.125.504-1.125
            1.125v12c0 .621.504 1.125 1.125 1.125h9.75c.621 0 1.125-.504 1.125-1.125V16.5a9 9 0 00-9-9z"></path></svg>`);
function st(i, e) {
  let t = S(e, "opacity", 8, 0.9), s = S(e, "width", 8, "1.5rem");
  var a = it();
  k(a, "stroke-width", 2), L(() => {
    k(a, "width", s()), k(a, "opacity", t());
  }), l(i, a);
}
var nt = ce(`<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M3.98 8.223A10.477 10.477 0 001.934 12C3.226 16.338 7.244 19.5 12 19.5c.993 0 1.953-.138
            2.863-.395M6.228 6.228A10.45 10.45 0 0112 4.5c4.756 0 8.773 3.162 10.065 7.498a10.523 10.523 0
            01-4.293 5.774M6.228 6.228L3 3m3.228 3.228l3.65 3.65m7.894 7.894L21 21m-3.228-3.228l-3.65-3.65m0
            0a3 3 0 10-4.243-4.243m4.242 4.242L9.88 9.88"></path></svg>`);
function lt(i, e) {
  let t = S(e, "color", 8, "var(--col-err)"), s = S(e, "opacity", 8, 0.9), a = S(e, "width", 8, "1.5rem");
  var o = nt();
  k(o, "stroke-width", 2), L(() => {
    k(o, "width", a()), k(o, "color", t()), k(o, "opacity", s());
  }), l(i, o);
}
var dt = p('<div role="button" tabindex="0" class="btn clip svelte-1gwcs78"><!></div>'), vt = p('<div class="nolabel svelte-1gwcs78"></div>'), ct = p('<div class="error svelte-1gwcs78"><!> </div>'), ut = p('<div><div class="input-row svelte-1gwcs78"><input/> <div class="rel svelte-1gwcs78"><!> <div role="button" tabindex="0" class="btn show svelte-1gwcs78"><!></div></div></div></div> <div class="label svelte-1gwcs78"><label class="font-label noselect svelte-1gwcs78"> </label> <!></div>', 1);
function ft(i, e) {
  let t = S(e, "type", 7, "password"), s = S(e, "name", 3, "password"), a = S(e, "value", 7, ""), o = S(e, "label", 3, "Password"), j = S(e, "autocomplete", 3, "current-password"), y = S(e, "placeholder", 3, "Password"), w = S(e, "title", 3, "Password"), v = S(e, "disabled", 3, false), u = S(e, "min", 3, "14"), g = S(e, "max", 3, "128"), x = S(e, "required", 3, true), P = S(e, "width", 3, "inherit"), n = S(e, "showCopy", 3, false), b = A(false);
  function d() {
    navigator.clipboard ? navigator.clipboard.writeText(a()) : console.error("Copy to clipboard is only available in secure contexts");
  }
  function H() {
    t() === "password" ? t("text") : t("password");
  }
  function U(M) {
    var _a;
    const I = (_a = M == null ? void 0 : M.currentTarget) == null ? void 0 : _a.reportValidity();
    T(b, !I);
  }
  function re(M) {
  }
  function oe(M) {
    M.preventDefault(), T(b, true);
  }
  function h(M) {
    M.code;
  }
  var q = ut(), R = z(q);
  let $;
  var G = _(R), f = _(G);
  Be(f), f.__input = re, f.__keydown = h;
  let D;
  var J = m(f, 2), ee = _(J);
  {
    var ie = (M) => {
      var I = dt();
      I.__click = d, I.__keydown = d;
      var te = _(I);
      st(te, {}), c(I), l(M, I);
    };
    V(ee, (M) => {
      n() && M(ie);
    });
  }
  var K = m(ee, 2);
  K.__click = H, K.__keydown = H;
  var N = _(K);
  {
    var Y = (M) => {
      lt(M, { width: 22 });
    }, se = (M) => {
      Fe(M, { width: 22 });
    };
    V(N, (M) => {
      t() === "password" ? M(Y) : M(se, false);
    });
  }
  c(K), c(J), c(G), c(R);
  var me = m(R, 2), W = _(me), Le = _(W, true);
  c(W);
  var je = m(W, 2);
  {
    var Te = (M) => {
      var I = ct(), te = _(I);
      {
        var Ce = (ne) => {
          var Ie = vt();
          l(ne, Ie);
        };
        V(te, (ne) => {
          o() || ne(Ce);
        });
      }
      var qe = m(te);
      c(I), L(() => C(qe, ` ${w() ?? ""}`)), Z(3, I, () => Oe), l(M, I);
    };
    V(je, (M) => {
      r(b) && M(Te);
    });
  }
  c(me), L(() => {
    $ = _e(R, "", $, { width: P() }), k(f, "type", t()), k(f, "id", e.id), k(f, "name", s()), k(f, "title", w()), k(f, "aria-label", w()), k(f, "autocomplete", j()), k(f, "placeholder", y()), f.disabled = v(), f.required = x() || void 0, k(f, "maxlength", e.maxLength || void 0), k(f, "min", u() || void 0), k(f, "max", g() || void 0), k(f, "pattern", e.pattern || void 0), D = _e(f, "", D, { "padding-right": n() ? "55px" : "30px" }), k(W, "for", e.id), k(W, "data-required", x()), C(Le, o());
  }), le("invalid", f, oe), le("blur", f, U), Ue(f, a), l(i, q);
}
ue(["input", "keydown", "click"]);
var mt = p('<meta property="description" content="Hiqlite Login"/>'), _t = p('<div class="err"> </div>'), ht = p("<!> <!> <!>", 1), wt = p('<div class="container svelte-8ukb9p"><div class="login svelte-8ukb9p"><!></div></div>');
function gt(i, e) {
  O(e, true);
  const t = `${xe}/session`;
  let s = A(""), a = A(false);
  async function o(v, u) {
    T(s, ""), T(a, true), u.append("pow", "NoPowUntilSvelte5ErrorFixed");
    const g = await fetch(t, { method: "POST", headers: { "Content-type": "application/x-www-form-urlencoded" }, body: u });
    let x = await g.json();
    g.status === 200 ? ve.set(x) : T(s, Object.values(x)[0], true), T(a, false);
  }
  var j = wt();
  ye("8ukb9p", (v) => {
    var u = mt();
    Ae(() => {
      Ve.title = "Login";
    }), l(v, u);
  });
  var y = _(j), w = _(y);
  ot(w, { get action() {
    return t;
  }, onSubmit: o, children: (v, u) => {
    var g = ht(), x = z(g);
    ft(x, { id: "password", name: "password", autocomplete: "current-password", placeholder: "Password", title: "Valid Dashboard Password", required: true });
    var P = m(x, 2);
    pe(P, { type: "submit", level: 1, get isLoading() {
      return r(a);
    }, children: (d, H) => {
      E();
      var U = B("Login");
      l(d, U);
    }, $$slots: { default: true } });
    var n = m(P, 2);
    {
      var b = (d) => {
        var H = _t(), U = _(H, true);
        c(H), L(() => C(U, r(s))), l(d, H);
      };
      V(n, (d) => {
        r(s) && d(b);
      });
    }
    l(v, g);
  }, $$slots: { default: true } }), c(y), c(j), l(i, j), Q();
}
var X = ((i) => (i.Table = "table", i.Index = "index", i.Trigger = "view", i.View = "trigger", i))(X || {}), bt = p(" <br/>", 1), pt = p('<section class="svelte-11pn1l8"><h5 class="header"> <br/> </h5> <div class="sql font-mono svelte-11pn1l8"></div></section>');
function yt(i, e) {
  O(e, true);
  let t = ge(() => {
    var _a;
    return (_a = e.table.sql) == null ? void 0 : _a.split(`
`);
  });
  var s = pt(), a = _(s), o = _(a, true), j = m(o, 2);
  c(a);
  var y = m(a, 2);
  ke(y, 21, () => r(t), Qe, (w, v) => {
    E();
    var u = bt(), g = z(u, true);
    E(), L(() => C(g, r(v))), l(w, u);
  }), c(y), c(s), L(() => {
    C(o, e.table.name), C(j, ` ${e.table.typ ?? ""}: ${e.table.tbl_name ?? ""}`);
  }), l(i, s), Q();
}
var xt = p('<div role="button" tabindex="0"> </div>');
function ae(i, e) {
  O(e, true);
  let t = S(e, "viewSelected", 15);
  function s() {
    t(e.view);
  }
  var a = xt();
  a.__click = s, a.__keydown = s;
  var o = _(a, true);
  c(a), L(() => {
    Se(a, 1, Pe(t() === e.view ? "selected" : ""), "svelte-r1zbyt"), C(o, e.view);
  }), l(i, a), Q();
}
ue(["click", "keydown"]);
var kt = ce('<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M19.5 14.25v-2.625a3.375 3.375 0 00-3.375-3.375h-1.5A1.125 1.125 0 0113.5 7.125v-1.5a3.375 3.375 0 00-3.375-3.375H8.25m0 12.75h7.5m-7.5 3H12M10.5 2.25H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 00-9-9z"></path></svg>');
function St(i, e) {
  let t = S(e, "opacity", 8, 0.9), s = S(e, "width", 8, "1.5rem");
  var a = kt();
  k(a, "stroke-width", 2), L(() => {
    k(a, "width", s()), k(a, "opacity", t());
  }), l(i, a);
}
var Pt = p('<div class="err"> </div>'), Mt = p('<div role="button" tabindex="0" class="btn svelte-1ep5fn"><!></div>'), Lt = p('<div role="button" tabindex="0"><div> </div> <!></div>'), jt = p('<!> <div class="selector svelte-1ep5fn"><!> <!> <!> <!></div> <div class="tables svelte-1ep5fn"><!> <!></div>', 1);
function Tt(i, e) {
  O(e, true);
  let t = A(de([])), s = A(void 0), a = A(de(X.Table)), o = A(void 0);
  we(() => {
    j(r(a));
  });
  async function j(h) {
    let q = await Me(`/tables/${h}`);
    q.status === 200 ? T(t, await q.json(), true) : T(o, await q.json(), true);
  }
  function y(h) {
    T(s, r(t).filter((q) => q.name === h)[0], true);
  }
  function w(h) {
    let q = { id: `${h}_${Ge(4)}`, query: `${Ke}
PRAGMA table_info(${h})` };
    Je.push(q), y(h);
  }
  var v = jt(), u = z(v);
  {
    var g = (h) => {
      var q = Pt(), R = _(q, true);
      c(q), L(() => C(R, r(o))), l(h, q);
    };
    V(u, (h) => {
      r(o) && h(g);
    });
  }
  var x = m(u, 2), P = _(x);
  ae(P, { get view() {
    return X.Table;
  }, get viewSelected() {
    return r(a);
  }, set viewSelected(h) {
    T(a, h, true);
  } });
  var n = m(P, 2);
  ae(n, { get view() {
    return X.Index;
  }, get viewSelected() {
    return r(a);
  }, set viewSelected(h) {
    T(a, h, true);
  } });
  var b = m(n, 2);
  ae(b, { get view() {
    return X.Trigger;
  }, get viewSelected() {
    return r(a);
  }, set viewSelected(h) {
    T(a, h, true);
  } });
  var d = m(b, 2);
  ae(d, { get view() {
    return X.View;
  }, get viewSelected() {
    return r(a);
  }, set viewSelected(h) {
    T(a, h, true);
  } }), c(x);
  var H = m(x, 2), U = _(H);
  Ye(U, { resizeBottom: true, initialHeightPx: window ? window.innerHeight - 400 : 600, minHeightPx: 120, children: (h, q) => {
    var R = he(), $ = z(R);
    ke($, 17, () => r(t), (G) => G.name, (G, f) => {
      var D = Lt();
      D.__click = () => y(r(f).name), D.__keydown = () => y(r(f).name);
      var J = _(D), ee = _(J, true);
      c(J);
      var ie = m(J, 2);
      {
        var K = (N) => {
          var Y = Mt();
          Y.__click = () => w(r(f).name), Y.__keydown = () => w(r(f).name);
          var se = _(Y);
          St(se, {}), c(Y), l(N, Y);
        };
        V(ie, (N) => {
          r(f).typ === "table" && N(K);
        });
      }
      c(D), L(() => {
        var _a;
        Se(D, 1, Pe(((_a = r(s)) == null ? void 0 : _a.name) === r(f).name ? "entry selected" : "entry"), "svelte-1ep5fn"), C(ee, r(f).name);
      }), l(G, D);
    }), l(h, R);
  }, $$slots: { default: true } });
  var re = m(U, 2);
  {
    var oe = (h) => {
      yt(h, { get table() {
        return r(s);
      } });
    };
    V(re, (h) => {
      r(s) && h(oe);
    });
  }
  c(H), l(i, v), Q();
}
ue(["click", "keydown"]);
var Ct = p('<div class="metric svelte-lz2k5j"><div class="label font-label svelte-lz2k5j"> </div> <div class="font-mono"><!></div></div>');
function F(i, e) {
  var t = Ct(), s = _(t), a = _(s, true);
  c(s);
  var o = m(s, 2), j = _(o);
  fe(j, () => e.children), c(o), c(t), L(() => C(a, e.label)), l(i, t);
}
var qt = p('<b>Metrics</b> <div class="space svelte-1xgy1jo"></div> <!> <!> <!> <!> <!> <!> <!> <!>', 1);
function It(i, e) {
  O(e, true);
  let t = A(void 0), s = ge(() => {
    var _a;
    return (_a = r(t)) == null ? void 0 : _a.membership_config.membership.configs.join(", ");
  });
  setInterval(() => {
    a();
  }, 1e4), be(() => {
    a();
  });
  async function a() {
    let n = await Me("/metrics");
    n.status === 200 ? T(t, await n.json(), true) : console.error(await n.json());
  }
  var o = qt(), j = m(z(o), 4);
  F(j, { label: "This Node", children: (n, b) => {
    E();
    var d = B();
    L(() => {
      var _a, _b;
      return C(d, `${((_a = r(t)) == null ? void 0 : _a.id) ?? ""}
    ${((_b = r(t)) == null ? void 0 : _b.state) ?? ""}`);
    }), l(n, d);
  } });
  var y = m(j, 2);
  F(y, { label: "Current Leader", children: (n, b) => {
    E();
    var d = B();
    L(() => {
      var _a;
      return C(d, (_a = r(t)) == null ? void 0 : _a.current_leader);
    }), l(n, d);
  } });
  var w = m(y, 2);
  F(w, { label: "Vote Leader", children: (n, b) => {
    E();
    var d = B();
    L(() => {
      var _a;
      return C(d, (_a = r(t)) == null ? void 0 : _a.vote.leader_id.node_id);
    }), l(n, d);
  } });
  var v = m(w, 2);
  F(v, { label: "Last Log Index", children: (n, b) => {
    E();
    var d = B();
    L(() => {
      var _a;
      return C(d, (_a = r(t)) == null ? void 0 : _a.last_log_index);
    }), l(n, d);
  } });
  var u = m(v, 2);
  F(u, { label: "Last Applied Log", children: (n, b) => {
    E();
    var d = B();
    L(() => {
      var _a, _b, _c, _d, _e2, _f;
      return C(d, `${((_b = (_a = r(t)) == null ? void 0 : _a.last_applied) == null ? void 0 : _b.leader_id.node_id) ?? ""}
    -
    ${((_d = (_c = r(t)) == null ? void 0 : _c.last_applied) == null ? void 0 : _d.leader_id.term) ?? ""}
    -
    ${((_f = (_e2 = r(t)) == null ? void 0 : _e2.last_applied) == null ? void 0 : _f.index) ?? ""}`);
    }), l(n, d);
  } });
  var g = m(u, 2);
  F(g, { label: "Last Snapshot", children: (n, b) => {
    E();
    var d = B();
    L(() => {
      var _a, _b, _c, _d;
      return C(d, `${((_b = (_a = r(t)) == null ? void 0 : _a.snapshot) == null ? void 0 : _b.leader_id) ?? ""}
    -
    ${((_d = (_c = r(t)) == null ? void 0 : _c.snapshot) == null ? void 0 : _d.index) ?? ""}`);
    }), l(n, d);
  } });
  var x = m(g, 2);
  F(x, { label: "Members", children: (n, b) => {
    E();
    var d = B();
    L(() => C(d, r(s))), l(n, d);
  } });
  var P = m(x, 2);
  F(P, { label: "Millis Quorum Ack", children: (n, b) => {
    E();
    var d = B();
    L(() => {
      var _a;
      return C(d, (_a = r(t)) == null ? void 0 : _a.millis_since_quorum_ack);
    }), l(n, d);
  } }), l(i, o), Q();
}
var At = p('<aside class="svelte-1u1auvt"><!></aside>');
function Vt(i) {
  var e = At(), t = _(e);
  It(t, {}), c(e), l(i, e);
}
const Et = (i, e, t) => {
  if (Ee(i)) return He(i);
  const s = e(t);
  return ze(i, s), s;
}, Ht = (i, e) => Et(i, zt, e), zt = (i) => {
  let e = A(de(i));
  return { get value() {
    return r(e);
  }, set value(t) {
    T(e, t, true);
  } };
};
var Rt = p('<meta name="robots" content="noindex nofollow"/>'), Dt = p('<nav class="svelte-12qhfyh"><!></nav> <main class="svelte-12qhfyh"><div class="inner svelte-12qhfyh"><!></div></main> <!>', 1), Bt = p("<!> <!>", 1);
function Jt(i, e) {
  O(e, true);
  let t = A(void 0), s = A(false);
  Ht("queries", [Xe]), ve.subscribe((v) => {
    T(t, v, true);
  }), be(async () => {
    let v = await fetch(`${xe}/session`);
    v.status === 200 && ve.set(await v.json()), T(s, true);
  });
  var a = Bt();
  ye("12qhfyh", (v) => {
    var u = Rt();
    l(v, u);
  });
  var o = z(a);
  {
    var j = (v) => {
      var u = Dt(), g = z(u), x = _(g);
      Tt(x, {}), c(g);
      var P = m(g, 2), n = _(P), b = _(n);
      fe(b, () => e.children), c(n), c(P);
      var d = m(P, 2);
      Vt(d), l(v, u);
    }, y = (v) => {
      var u = he(), g = z(u);
      {
        var x = (P) => {
          gt(P, {});
        };
        V(g, (P) => {
          r(s) && P(x);
        }, true);
      }
      l(v, u);
    };
    V(o, (v) => {
      r(t) ? v(j) : v(y, false);
    });
  }
  var w = m(o, 2);
  at(w), l(i, a), Q();
}
export {
  Jt as component,
  Gt as universal
};
