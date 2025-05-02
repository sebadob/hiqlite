import { t as y, a as l, p as ne, o as ve, e as ce, s as C, q as we, b as B, c as qe } from "../chunks/wD4zbyrH.js";
import { p as Q, a as ge, j as F, g as r, a3 as I, N as M, i as U, s as _, k as m, l as u, t as P, aO as Ie, aP as E, a5 as be, W as le, a2 as pe, aQ as Ae, aR as Ve, aS as Ee } from "../chunks/BydrjbDF.js";
import { i as A, p as S } from "../chunks/BC66Giei.js";
import { B as ye, t as J, c as He, s as ue, a as x, h as Re, r as ze, b as _e, d as De, I as Be, e as Ue, A as ke, f as de, g as xe, i as Oe, j as Se, k as Pe, l as je, R as Qe, m as Fe, Q as Ge, n as Ye, D as Ke } from "../chunks/CmsUE4a5.js";
import "../chunks/4j7-kMch.js";
const Ne = true, Xt = Object.freeze(Object.defineProperty({ __proto__: null, prerender: Ne }, Symbol.toStringTag, { value: "Module" }));
var We = y(`<div class="icon moon svelte-b827j5"><svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor" class="w-6 h-6"><path stroke-linecap="round" stroke-linejoin="round" d="M21.752 15.002A9.718 9.718 0 0118 15.75c-5.385 0-9.75-4.365-9.75-9.75
                        0-1.33.266-2.597.748-3.752A9.753 9.753 0 003 11.25C3 16.635 7.365 21 12.75
                        21a9.753 9.753 0 009.002-5.998z"></path></svg></div>`), Xe = y(`<div class="icon sun svelte-b827j5"><svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor" class="w-6 h-6"><path stroke-linecap="round" stroke-linejoin="round" d="M12 3v2.25m6.364.386l-1.591 1.591M21 12h-2.25m-.386 6.364l-1.591-1.591M12
                        18.75V21m-4.773-4.227l-1.591 1.591M5.25 12H3m4.227-4.773L5.636 5.636M15.75 12a3.75
                        3.75 0 11-7.5 0 3.75 3.75 0 017.5 0z"></path></svg></div>`), Je = y("<!> <!>", 1);
function Ze(a, e) {
  Q(e, true);
  const t = "darkMode", [i, o] = He({});
  let s = I(void 0);
  ge(() => {
    var _a, _b, _c;
    const k = ((_b = (_a = window == null ? void 0 : window.matchMedia) == null ? void 0 : _a.call(window, "(prefers-color-scheme:dark)")) == null ? void 0 : _b.matches) ?? false;
    if (r(s) === void 0) {
      const g = (_c = window == null ? void 0 : window.localStorage) == null ? void 0 : _c.getItem(t);
      if (g) {
        let v = g === "true";
        r(s) === k && localStorage.removeItem(t), M(s, v);
      } else M(s, k, true);
    } else r(s) ? (document.body.classList.remove("light-theme"), document.body.classList.add("dark-theme")) : (document.body.classList.remove("dark-theme"), document.body.classList.add("light-theme"));
    k === r(s) ? localStorage.removeItem(t) : localStorage.setItem(t, r(s).toString());
  });
  function j() {
    M(s, !r(s));
  }
  ye(a, { ariaLabel: "Change color theme", invisible: true, onclick: j, children: (k, g) => {
    var v = Je(), f = U(v);
    {
      var b = (n) => {
        var p = We();
        J(1, p, () => o, () => ({ key: "dark" })), J(2, p, () => i, () => ({ key: "light" })), l(n, p);
      };
      A(f, (n) => {
        r(s) === true && n(b);
      });
    }
    var w = _(f, 2);
    {
      var T = (n) => {
        var p = Xe();
        J(1, p, () => o, () => ({ key: "light" })), J(2, p, () => i, () => ({ key: "dark" })), l(n, p);
      };
      A(w, (n) => {
        r(s) === false && n(T);
      });
    }
    l(k, v);
  }, $$slots: { default: true } }), F();
}
var $e = y('<div class="theme-switch svelte-jrz9as"><!></div>');
function et(a) {
  var e = $e(), t = m(e);
  Ze(t, {}), u(e), l(a, e);
}
var tt = y("<form><!></form>");
function at(a, e) {
  Q(e, true);
  let t = S(e, "method", 3, "POST"), i = S(e, "isError", 15);
  async function o(k) {
    k.preventDefault();
    const g = k.currentTarget;
    if (g.reportValidity()) i(false);
    else {
      i(true);
      return;
    }
    const f = new FormData(g);
    let b = new URLSearchParams();
    if (f.forEach((T, n) => {
      b.append(n, T.toString());
    }), e.onSubmit) {
      e.onSubmit(g, b);
      return;
    }
    const w = await fetch(g.action, { method: g.method, headers: { "Content-type": "application/x-www-form-urlencoded" }, body: b });
    Re(w), e.onResponse && (e.onResponse(w), w.ok && g.reset());
  }
  var s = tt(), j = m(s);
  ue(j, () => e.children), u(s), P(() => {
    x(s, "action", e.action), x(s, "method", t());
  }), ne("submit", s, o), l(a, s), F();
}
var rt = ve(`<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M8.25 7.5V6.108c0-1.135.845-2.098 1.976-2.192.373-.03.748-.057 1.123-.08M15.75 18H18a2.25 2.25 0
            002.25-2.25V6.108c0-1.135-.845-2.098-1.976-2.192a48.424 48.424 0 00-1.123-.08M15.75 18.75v-1.875a3.375
            3.375 0 00-3.375-3.375h-1.5a1.125 1.125 0 01-1.125-1.125v-1.5A3.375 3.375 0 006.375
            7.5H5.25m11.9-3.664A2.251 2.251 0 0015 2.25h-1.5a2.251 2.251 0 00-2.15 1.586m5.8
            0c.065.21.1.433.1.664v.75h-6V4.5c0-.231.035-.454.1-.664M6.75 7.5H4.875c-.621 0-1.125.504-1.125
            1.125v12c0 .621.504 1.125 1.125 1.125h9.75c.621 0 1.125-.504 1.125-1.125V16.5a9 9 0 00-9-9z"></path></svg>`);
function ot(a, e) {
  let t = S(e, "opacity", 8, 0.9), i = S(e, "width", 8, "1.5rem");
  var o = rt();
  x(o, "stroke-width", 2), P(() => {
    x(o, "width", i()), x(o, "opacity", t());
  }), l(a, o);
}
var it = ve(`<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M3.98 8.223A10.477 10.477 0 001.934 12C3.226 16.338 7.244 19.5 12 19.5c.993 0 1.953-.138
            2.863-.395M6.228 6.228A10.45 10.45 0 0112 4.5c4.756 0 8.773 3.162 10.065 7.498a10.523 10.523 0
            01-4.293 5.774M6.228 6.228L3 3m3.228 3.228l3.65 3.65m7.894 7.894L21 21m-3.228-3.228l-3.65-3.65m0
            0a3 3 0 10-4.243-4.243m4.242 4.242L9.88 9.88"></path></svg>`);
function st(a, e) {
  let t = S(e, "color", 8, "var(--col-err)"), i = S(e, "opacity", 8, 0.9), o = S(e, "width", 8, "1.5rem");
  var s = it();
  x(s, "stroke-width", 2), P(() => {
    x(s, "width", o()), x(s, "color", t()), x(s, "opacity", i());
  }), l(a, s);
}
function me(a, e) {
  navigator.clipboard ? navigator.clipboard.writeText(e()) : console.error("Copy to clipboard is only available in secure contexts");
}
function fe(a, e) {
  e() === "password" ? e("text") : e("password");
}
function nt(a) {
}
function lt(a) {
  a.code;
}
var dt = y('<div role="button" tabindex="0" class="btn clip svelte-11t06s4"><!></div>'), vt = y('<div class="nolabel svelte-11t06s4"></div>'), ct = y('<div class="error svelte-11t06s4"><!> </div>'), ut = y('<div><div class="input-row svelte-11t06s4"><input> <div class="rel svelte-11t06s4"><!> <div role="button" tabindex="0" class="btn show svelte-11t06s4"><!></div></div></div></div> <div class="label svelte-11t06s4"><label class="font-label noselect svelte-11t06s4"> </label> <!></div>', 1);
function _t(a, e) {
  let t = S(e, "type", 7, "password"), i = S(e, "name", 3, "password"), o = S(e, "value", 7, ""), s = S(e, "label", 3, "Password"), j = S(e, "autocomplete", 3, "current-password"), k = S(e, "placeholder", 3, "Password"), g = S(e, "title", 3, "Password"), v = S(e, "disabled", 3, false), f = S(e, "min", 3, "14"), b = S(e, "max", 3, "128"), w = S(e, "required", 3, true), T = S(e, "width", 3, "inherit"), n = S(e, "showCopy", 3, false), p = I(false);
  function d(L) {
    var _a;
    const q = (_a = L == null ? void 0 : L.currentTarget) == null ? void 0 : _a.reportValidity();
    M(p, !q);
  }
  function H(L) {
    L.preventDefault(), M(p, true);
  }
  var R = ut(), K = U(R);
  let Z;
  var h = m(K), c = m(h);
  ze(c), c.__input = [nt], c.__keydown = [lt];
  let G;
  var $ = _(c, 2), N = m($);
  {
    var z = (L) => {
      var q = dt();
      q.__click = [me, o], q.__keydown = [me, o];
      var te = m(q);
      ot(te, {}), u(q), l(L, q);
    };
    A(N, (L) => {
      n() && L(z);
    });
  }
  var V = _(N, 2);
  V.__click = [fe, t], V.__keydown = [fe, t];
  var X = m(V);
  {
    var re = (L) => {
      st(L, { width: 22 });
    }, oe = (L) => {
      Be(L, { width: 22 });
    };
    A(X, (L) => {
      t() === "password" ? L(re) : L(oe, false);
    });
  }
  u(V), u($), u(h), u(K);
  var ee = _(K, 2), D = m(ee), Y = m(D, true);
  u(D);
  var ie = _(D, 2);
  {
    var Me = (L) => {
      var q = ct(), te = m(q);
      {
        var Le = (se) => {
          var Te = vt();
          l(se, Te);
        };
        A(te, (se) => {
          s() || se(Le);
        });
      }
      var Ce = _(te);
      u(q), P(() => C(Ce, ` ${g() ?? ""}`)), J(3, q, () => Ue), l(L, q);
    };
    A(ie, (L) => {
      r(p) && L(Me);
    });
  }
  u(ee), P(() => {
    Z = _e(K, "", Z, { width: T() }), x(c, "type", t()), x(c, "id", e.id), x(c, "name", i()), x(c, "title", g()), x(c, "aria-label", g()), x(c, "autocomplete", j()), x(c, "placeholder", k()), c.disabled = v(), c.required = w() || void 0, x(c, "maxlength", e.maxLength || void 0), x(c, "min", f() || void 0), x(c, "max", b() || void 0), x(c, "pattern", e.pattern || void 0), G = _e(c, "", G, { "padding-right": n() ? "55px" : "30px" }), x(D, "for", e.id), x(D, "data-required", w()), C(Y, s());
  }), ne("invalid", c, H), ne("blur", c, d), De(c, o), l(a, R);
}
ce(["input", "keydown", "click"]);
var mt = y('<meta property="description" content="Hiqlite Login">'), ft = y('<div class="err"> </div>'), ht = y("<!> <!> <!>", 1), wt = y('<div class="container svelte-dc3gug"><div class="login svelte-dc3gug"><!></div></div>');
function gt(a, e) {
  Q(e, true);
  const t = `${ke}/session`;
  let i = I(""), o = I(false);
  async function s(v, f) {
    M(i, ""), M(o, true), f.append("pow", "NoPowUntilSvelte5ErrorFixed");
    const b = await fetch(t, { method: "POST", headers: { "Content-type": "application/x-www-form-urlencoded" }, body: f });
    let w = await b.json();
    b.status === 200 ? de.set(w) : M(i, Object.values(w)[0], true), M(o, false);
  }
  var j = wt();
  we((v) => {
    var f = mt();
    Ie.title = "Login", l(v, f);
  });
  var k = m(j), g = m(k);
  at(g, { action: t, onSubmit: s, children: (v, f) => {
    var b = ht(), w = U(b);
    _t(w, { id: "password", name: "password", autocomplete: "current-password", placeholder: "Password", title: "Valid Dashboard Password", required: true });
    var T = _(w, 2);
    ye(T, { type: "submit", level: 1, get isLoading() {
      return r(o);
    }, children: (d, H) => {
      E();
      var R = B("Login");
      l(d, R);
    }, $$slots: { default: true } });
    var n = _(T, 2);
    {
      var p = (d) => {
        var H = ft(), R = m(H, true);
        u(H), P(() => C(R, r(i))), l(d, H);
      };
      A(n, (d) => {
        r(i) && d(p);
      });
    }
    l(v, b);
  }, $$slots: { default: true } }), u(k), u(j), l(a, j), F();
}
var W = ((a) => (a.Table = "table", a.Index = "index", a.Trigger = "view", a.View = "trigger", a))(W || {}), bt = y(" <br>", 1), pt = y('<section class="svelte-y9kii"><h5 class="header"> <br> </h5> <div class="sql font-mono svelte-y9kii"></div></section>');
function yt(a, e) {
  Q(e, true);
  let t = be(() => {
    var _a;
    return (_a = e.table.sql) == null ? void 0 : _a.split(`
`);
  });
  var i = pt(), o = m(i), s = m(o, true), j = _(s, 2);
  u(o);
  var k = _(o, 2);
  xe(k, 21, () => r(t), Oe, (g, v) => {
    E();
    var f = bt(), b = U(f, true);
    E(), P(() => C(b, r(v))), l(g, f);
  }), u(k), u(i), P(() => {
    C(s, e.table.name), C(j, ` ${e.table.typ ?? ""}: ${e.table.tbl_name ?? ""}`);
  }), l(a, i), F();
}
function he(a, e, t) {
  e(t.view);
}
var kt = y('<div role="button" tabindex="0"> </div>');
function ae(a, e) {
  Q(e, true);
  let t = S(e, "viewSelected", 15);
  var i = kt();
  i.__click = [he, t, e], i.__keydown = [he, t, e];
  var o = m(i, true);
  u(i), P(() => {
    Se(i, 1, Pe(t() === e.view ? "selected" : ""), "svelte-13ofdkm"), C(o, e.view);
  }), l(a, i), F();
}
ce(["click", "keydown"]);
var xt = ve('<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M19.5 14.25v-2.625a3.375 3.375 0 00-3.375-3.375h-1.5A1.125 1.125 0 0113.5 7.125v-1.5a3.375 3.375 0 00-3.375-3.375H8.25m0 12.75h7.5m-7.5 3H12M10.5 2.25H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 00-9-9z"></path></svg>');
function St(a, e) {
  let t = S(e, "opacity", 8, 0.9), i = S(e, "width", 8, "1.5rem");
  var o = xt();
  x(o, "stroke-width", 2), P(() => {
    x(o, "width", i()), x(o, "opacity", t());
  }), l(a, o);
}
var Pt = y('<div class="err"> </div>'), jt = (a, e, t) => e(r(t).name), Mt = (a, e, t) => e(r(t).name), Lt = (a, e, t) => e(r(t).name), Ct = (a, e, t) => e(r(t).name), Tt = y('<div role="button" tabindex="0" class="btn svelte-trjj59"><!></div>'), qt = y('<div role="button" tabindex="0"><div> </div> <!></div>'), It = y('<!> <div class="selector svelte-trjj59"><!> <!> <!> <!></div> <div class="tables svelte-trjj59"><!> <!></div>', 1);
function At(a, e) {
  Q(e, true);
  let t = I(le([])), i = I(void 0), o = I(le(W.Table)), s = I(void 0);
  ge(() => {
    j(r(o));
  });
  async function j(h) {
    let c = await je(`/tables/${h}`);
    c.status === 200 ? M(t, await c.json(), true) : M(s, await c.json(), true);
  }
  function k(h) {
    M(i, r(t).filter((c) => c.name === h)[0], true);
  }
  function g(h) {
    let c = { id: `${h}_${Fe(4)}`, query: `${Ye}
PRAGMA table_info(${h})` };
    Ge.push(c), k(h);
  }
  var v = It(), f = U(v);
  {
    var b = (h) => {
      var c = Pt(), G = m(c, true);
      u(c), P(() => C(G, r(s))), l(h, c);
    };
    A(f, (h) => {
      r(s) && h(b);
    });
  }
  var w = _(f, 2), T = m(w);
  ae(T, { get view() {
    return W.Table;
  }, get viewSelected() {
    return r(o);
  }, set viewSelected(h) {
    M(o, h, true);
  } });
  var n = _(T, 2);
  ae(n, { get view() {
    return W.Index;
  }, get viewSelected() {
    return r(o);
  }, set viewSelected(h) {
    M(o, h, true);
  } });
  var p = _(n, 2);
  ae(p, { get view() {
    return W.Trigger;
  }, get viewSelected() {
    return r(o);
  }, set viewSelected(h) {
    M(o, h, true);
  } });
  var d = _(p, 2);
  ae(d, { get view() {
    return W.View;
  }, get viewSelected() {
    return r(o);
  }, set viewSelected(h) {
    M(o, h, true);
  } }), u(w);
  var H = _(w, 2), R = m(H);
  Qe(R, { resizeBottom: true, initialHeightPx: window ? window.innerHeight - 400 : 600, minHeightPx: 120, children: (h, c) => {
    var G = qe(), $ = U(G);
    xe($, 17, () => r(t), (N) => N.name, (N, z) => {
      var V = qt();
      V.__click = [jt, k, z], V.__keydown = [Mt, k, z];
      var X = m(V), re = m(X, true);
      u(X);
      var oe = _(X, 2);
      {
        var ee = (D) => {
          var Y = Tt();
          Y.__click = [Lt, g, z], Y.__keydown = [Ct, g, z];
          var ie = m(Y);
          St(ie, {}), u(Y), l(D, Y);
        };
        A(oe, (D) => {
          r(z).typ === "table" && D(ee);
        });
      }
      u(V), P(() => {
        var _a;
        Se(V, 1, Pe(((_a = r(i)) == null ? void 0 : _a.name) === r(z).name ? "entry selected" : "entry"), "svelte-trjj59"), C(re, r(z).name);
      }), l(N, V);
    }), l(h, G);
  }, $$slots: { default: true } });
  var K = _(R, 2);
  {
    var Z = (h) => {
      yt(h, { get table() {
        return r(i);
      } });
    };
    A(K, (h) => {
      r(i) && h(Z);
    });
  }
  u(H), l(a, v), F();
}
ce(["click", "keydown"]);
var Vt = y('<div class="metric svelte-1ktnipf"><div class="label font-label svelte-1ktnipf"> </div> <div class="font-mono"><!></div></div>');
function O(a, e) {
  var t = Vt(), i = m(t), o = m(i, true);
  u(i);
  var s = _(i, 2), j = m(s);
  ue(j, () => e.children), u(s), u(t), P(() => C(o, e.label)), l(a, t);
}
var Et = y('<b>Metrics</b> <div class="space svelte-12lemcq"></div> <!> <!> <!> <!> <!> <!> <!> <!>', 1);
function Ht(a, e) {
  Q(e, true);
  let t = I(void 0), i = be(() => {
    var _a;
    return (_a = r(t)) == null ? void 0 : _a.membership_config.membership.configs.join(", ");
  });
  setInterval(() => {
    o();
  }, 1e4), pe(() => {
    o();
  });
  async function o() {
    let n = await je("/metrics");
    n.status === 200 ? M(t, await n.json(), true) : console.error(await n.json());
  }
  var s = Et(), j = _(U(s), 4);
  O(j, { label: "This Node", children: (n, p) => {
    E();
    var d = B();
    P(() => {
      var _a, _b;
      return C(d, `${((_a = r(t)) == null ? void 0 : _a.id) ?? ""}
    ${((_b = r(t)) == null ? void 0 : _b.state) ?? ""}`);
    }), l(n, d);
  } });
  var k = _(j, 2);
  O(k, { label: "Current Leader", children: (n, p) => {
    E();
    var d = B();
    P(() => {
      var _a;
      return C(d, (_a = r(t)) == null ? void 0 : _a.current_leader);
    }), l(n, d);
  } });
  var g = _(k, 2);
  O(g, { label: "Vote Leader", children: (n, p) => {
    E();
    var d = B();
    P(() => {
      var _a;
      return C(d, (_a = r(t)) == null ? void 0 : _a.vote.leader_id.node_id);
    }), l(n, d);
  } });
  var v = _(g, 2);
  O(v, { label: "Last Log Index", children: (n, p) => {
    E();
    var d = B();
    P(() => {
      var _a;
      return C(d, (_a = r(t)) == null ? void 0 : _a.last_log_index);
    }), l(n, d);
  } });
  var f = _(v, 2);
  O(f, { label: "Last Applied Log", children: (n, p) => {
    E();
    var d = B();
    P(() => {
      var _a, _b, _c, _d, _e2, _f;
      return C(d, `${((_b = (_a = r(t)) == null ? void 0 : _a.last_applied) == null ? void 0 : _b.leader_id.node_id) ?? ""}
    -
    ${((_d = (_c = r(t)) == null ? void 0 : _c.last_applied) == null ? void 0 : _d.leader_id.term) ?? ""}
    -
    ${((_f = (_e2 = r(t)) == null ? void 0 : _e2.last_applied) == null ? void 0 : _f.index) ?? ""}`);
    }), l(n, d);
  } });
  var b = _(f, 2);
  O(b, { label: "Last Snapshot", children: (n, p) => {
    E();
    var d = B();
    P(() => {
      var _a, _b, _c, _d;
      return C(d, `${((_b = (_a = r(t)) == null ? void 0 : _a.snapshot) == null ? void 0 : _b.leader_id) ?? ""}
    -
    ${((_d = (_c = r(t)) == null ? void 0 : _c.snapshot) == null ? void 0 : _d.index) ?? ""}`);
    }), l(n, d);
  } });
  var w = _(b, 2);
  O(w, { label: "Members", children: (n, p) => {
    E();
    var d = B();
    P(() => C(d, r(i))), l(n, d);
  } });
  var T = _(w, 2);
  O(T, { label: "Millis Quorum Ack", children: (n, p) => {
    E();
    var d = B();
    P(() => {
      var _a;
      return C(d, (_a = r(t)) == null ? void 0 : _a.millis_since_quorum_ack);
    }), l(n, d);
  } }), l(a, s), F();
}
var Rt = y('<aside class="svelte-154rhoy"><!></aside>');
function zt(a) {
  var e = Rt(), t = m(e);
  Ht(t, {}), u(e), l(a, e);
}
const Dt = (a, e, t) => {
  if (Ae(a)) return Ve(a);
  const i = e(t);
  return Ee(a, i), i;
}, Bt = (a, e) => Dt(a, Ut, e), Ut = (a) => {
  let e = I(le(a));
  return { get value() {
    return r(e);
  }, set value(t) {
    M(e, t, true);
  } };
};
var Ot = y('<meta name="robots" content="noindex nofollow">'), Qt = y('<nav class="svelte-vv6eq"><!></nav> <main class="svelte-vv6eq"><div class="inner svelte-vv6eq"><!></div></main> <!>', 1), Ft = y("<!> <!>", 1);
function Jt(a, e) {
  Q(e, true);
  let t = I(void 0), i = I(false);
  Bt("queries", [Ke]), de.subscribe((v) => {
    M(t, v, true);
  }), pe(async () => {
    let v = await fetch(`${ke}/session`);
    v.status === 200 && de.set(await v.json()), M(i, true);
  });
  var o = Ft();
  we((v) => {
    var f = Ot();
    l(v, f);
  });
  var s = U(o);
  {
    var j = (v) => {
      var f = Qt(), b = U(f), w = m(b);
      At(w, {}), u(b);
      var T = _(b, 2), n = m(T), p = m(n);
      ue(p, () => e.children), u(n), u(T);
      var d = _(T, 2);
      zt(d), l(v, f);
    }, k = (v, f) => {
      {
        var b = (w) => {
          gt(w, {});
        };
        A(v, (w) => {
          r(i) && w(b);
        }, f);
      }
    };
    A(s, (v) => {
      r(t) ? v(j) : v(k, false);
    });
  }
  var g = _(s, 2);
  et(g), l(a, o), F();
}
export {
  Jt as component,
  Xt as universal
};
