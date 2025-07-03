import "../chunks/NZTpNUN0.js";
import { p as O, a as we, l as Q, g as r, a8 as I, R as L, i as k, j as U, s as _, k as l, m, n as u, aZ as ve, t as P, b0 as se, a$ as ce, aG as ue, o as C, b1 as ge, b2 as qe, b3 as E, ac as B, ab as be, Y as le, aa as Ie, a7 as ye, b4 as Ae, b5 as Ve, b6 as Ee } from "../chunks/CYo-iuqb.js";
import { i as A, p as S } from "../chunks/mITizLRE.js";
import "../chunks/BXesWDf4.js";
import { B as ke, t as N, c as He, s as x, h as Re, r as ze, a as _e, b as De, I as Be, d as Ue, A as pe, e as de, f as xe, i as Fe, g as Se, j as Pe, k as je, R as Oe, l as Qe, Q as Ge, m as Ye, D as Ke } from "../chunks/Bc67SYVN.js";
const Xe = true, Jt = Object.freeze(Object.defineProperty({ __proto__: null, prerender: Xe }, Symbol.toStringTag, { value: "Module" }));
var Ze = k(`<div class="icon moon svelte-b827j5"><svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor" class="w-6 h-6"><path stroke-linecap="round" stroke-linejoin="round" d="M21.752 15.002A9.718 9.718 0 0118 15.75c-5.385 0-9.75-4.365-9.75-9.75
                        0-1.33.266-2.597.748-3.752A9.753 9.753 0 003 11.25C3 16.635 7.365 21 12.75
                        21a9.753 9.753 0 009.002-5.998z"></path></svg></div>`), Je = k(`<div class="icon sun svelte-b827j5"><svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor" class="w-6 h-6"><path stroke-linecap="round" stroke-linejoin="round" d="M12 3v2.25m6.364.386l-1.591 1.591M21 12h-2.25m-.386 6.364l-1.591-1.591M12
                        18.75V21m-4.773-4.227l-1.591 1.591M5.25 12H3m4.227-4.773L5.636 5.636M15.75 12a3.75
                        3.75 0 11-7.5 0 3.75 3.75 0 017.5 0z"></path></svg></div>`), Ne = k("<!> <!>", 1);
function We(a, e) {
  O(e, true);
  const t = "darkMode", [i, o] = He({});
  let n = I(void 0);
  we(() => {
    var _a, _b, _c;
    const p = ((_b = (_a = window == null ? void 0 : window.matchMedia) == null ? void 0 : _a.call(window, "(prefers-color-scheme:dark)")) == null ? void 0 : _b.matches) ?? false;
    if (r(n) === void 0) {
      const g = (_c = window == null ? void 0 : window.localStorage) == null ? void 0 : _c.getItem(t);
      if (g) {
        let v = g === "true";
        r(n) === p && localStorage.removeItem(t), L(n, v);
      } else L(n, p, true);
    } else r(n) ? (document.body.classList.remove("light-theme"), document.body.classList.add("dark-theme")) : (document.body.classList.remove("dark-theme"), document.body.classList.add("light-theme"));
    p === r(n) ? localStorage.removeItem(t) : localStorage.setItem(t, r(n).toString());
  });
  function j() {
    L(n, !r(n));
  }
  ke(a, { ariaLabel: "Change color theme", invisible: true, onclick: j, children: (p, g) => {
    var v = Ne(), f = U(v);
    {
      var b = (s) => {
        var y = Ze();
        N(1, y, () => o, () => ({ key: "dark" })), N(2, y, () => i, () => ({ key: "light" })), l(s, y);
      };
      A(f, (s) => {
        r(n) === true && s(b);
      });
    }
    var w = _(f, 2);
    {
      var T = (s) => {
        var y = Je();
        N(1, y, () => o, () => ({ key: "light" })), N(2, y, () => i, () => ({ key: "dark" })), l(s, y);
      };
      A(w, (s) => {
        r(n) === false && s(T);
      });
    }
    l(p, v);
  }, $$slots: { default: true } }), Q();
}
var $e = k('<div class="theme-switch svelte-jrz9as"><!></div>');
function et(a) {
  var e = $e(), t = m(e);
  We(t, {}), u(e), l(a, e);
}
var tt = k("<form><!></form>");
function at(a, e) {
  O(e, true);
  let t = S(e, "method", 3, "POST"), i = S(e, "isError", 15);
  async function o(p) {
    p.preventDefault();
    const g = p.currentTarget;
    if (g.reportValidity()) i(false);
    else {
      i(true);
      return;
    }
    const f = new FormData(g);
    let b = new URLSearchParams();
    if (f.forEach((T, s) => {
      b.append(s, T.toString());
    }), e.onSubmit) {
      e.onSubmit(g, b);
      return;
    }
    const w = await fetch(g.action, { method: g.method, headers: { "Content-type": "application/x-www-form-urlencoded" }, body: b });
    Re(w), e.onResponse && (e.onResponse(w), w.ok && g.reset());
  }
  var n = tt(), j = m(n);
  ve(j, () => e.children), u(n), P(() => {
    x(n, "action", e.action), x(n, "method", t());
  }), se("submit", n, o), l(a, n), Q();
}
var rt = ce(`<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M8.25 7.5V6.108c0-1.135.845-2.098 1.976-2.192.373-.03.748-.057 1.123-.08M15.75 18H18a2.25 2.25 0
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
var it = ce(`<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M3.98 8.223A10.477 10.477 0 001.934 12C3.226 16.338 7.244 19.5 12 19.5c.993 0 1.953-.138
            2.863-.395M6.228 6.228A10.45 10.45 0 0112 4.5c4.756 0 8.773 3.162 10.065 7.498a10.523 10.523 0
            01-4.293 5.774M6.228 6.228L3 3m3.228 3.228l3.65 3.65m7.894 7.894L21 21m-3.228-3.228l-3.65-3.65m0
            0a3 3 0 10-4.243-4.243m4.242 4.242L9.88 9.88"></path></svg>`);
function nt(a, e) {
  let t = S(e, "color", 8, "var(--col-err)"), i = S(e, "opacity", 8, 0.9), o = S(e, "width", 8, "1.5rem");
  var n = it();
  x(n, "stroke-width", 2), P(() => {
    x(n, "width", o()), x(n, "color", t()), x(n, "opacity", i());
  }), l(a, n);
}
function me(a, e) {
  navigator.clipboard ? navigator.clipboard.writeText(e()) : console.error("Copy to clipboard is only available in secure contexts");
}
function fe(a, e) {
  e() === "password" ? e("text") : e("password");
}
function st(a) {
}
function lt(a) {
  a.code;
}
var dt = k('<div role="button" tabindex="0" class="btn clip svelte-11t06s4"><!></div>'), vt = k('<div class="nolabel svelte-11t06s4"></div>'), ct = k('<div class="error svelte-11t06s4"><!> </div>'), ut = k('<div><div class="input-row svelte-11t06s4"><input/> <div class="rel svelte-11t06s4"><!> <div role="button" tabindex="0" class="btn show svelte-11t06s4"><!></div></div></div></div> <div class="label svelte-11t06s4"><label class="font-label noselect svelte-11t06s4"> </label> <!></div>', 1);
function _t(a, e) {
  let t = S(e, "type", 7, "password"), i = S(e, "name", 3, "password"), o = S(e, "value", 7, ""), n = S(e, "label", 3, "Password"), j = S(e, "autocomplete", 3, "current-password"), p = S(e, "placeholder", 3, "Password"), g = S(e, "title", 3, "Password"), v = S(e, "disabled", 3, false), f = S(e, "min", 3, "14"), b = S(e, "max", 3, "128"), w = S(e, "required", 3, true), T = S(e, "width", 3, "inherit"), s = S(e, "showCopy", 3, false), y = I(false);
  function d(M) {
    var _a;
    const q = (_a = M == null ? void 0 : M.currentTarget) == null ? void 0 : _a.reportValidity();
    L(y, !q);
  }
  function H(M) {
    M.preventDefault(), L(y, true);
  }
  var R = ut(), K = U(R);
  let W;
  var h = m(K), c = m(h);
  ze(c), c.__input = [st], c.__keydown = [lt];
  let G;
  var $ = _(c, 2), X = m($);
  {
    var z = (M) => {
      var q = dt();
      q.__click = [me, o], q.__keydown = [me, o];
      var te = m(q);
      ot(te, {}), u(q), l(M, q);
    };
    A(X, (M) => {
      s() && M(z);
    });
  }
  var V = _(X, 2);
  V.__click = [fe, t], V.__keydown = [fe, t];
  var J = m(V);
  {
    var re = (M) => {
      nt(M, { width: 22 });
    }, oe = (M) => {
      Be(M, { width: 22 });
    };
    A(J, (M) => {
      t() === "password" ? M(re) : M(oe, false);
    });
  }
  u(V), u($), u(h), u(K);
  var ee = _(K, 2), D = m(ee), Y = m(D, true);
  u(D);
  var ie = _(D, 2);
  {
    var Me = (M) => {
      var q = ct(), te = m(q);
      {
        var Le = (ne) => {
          var Te = vt();
          l(ne, Te);
        };
        A(te, (ne) => {
          n() || ne(Le);
        });
      }
      var Ce = _(te);
      u(q), P(() => C(Ce, ` ${g() ?? ""}`)), N(3, q, () => Ue), l(M, q);
    };
    A(ie, (M) => {
      r(y) && M(Me);
    });
  }
  u(ee), P((M, q) => {
    W = _e(K, "", W, M), x(c, "type", t()), x(c, "id", e.id), x(c, "name", i()), x(c, "title", g()), x(c, "aria-label", g()), x(c, "autocomplete", j()), x(c, "placeholder", p()), c.disabled = v(), c.required = w() || void 0, x(c, "maxlength", e.maxLength || void 0), x(c, "min", f() || void 0), x(c, "max", b() || void 0), x(c, "pattern", e.pattern || void 0), G = _e(c, "", G, q), x(D, "for", e.id), x(D, "data-required", w()), C(Y, n());
  }, [() => ({ width: T() }), () => ({ "padding-right": s() ? "55px" : "30px" })]), se("invalid", c, H), se("blur", c, d), De(c, o), l(a, R);
}
ue(["input", "keydown", "click"]);
var mt = k('<meta property="description" content="Hiqlite Login"/>'), ft = k('<div class="err"> </div>'), ht = k("<!> <!> <!>", 1), wt = k('<div class="container svelte-dc3gug"><div class="login svelte-dc3gug"><!></div></div>');
function gt(a, e) {
  O(e, true);
  const t = `${pe}/session`;
  let i = I(""), o = I(false);
  async function n(v, f) {
    L(i, ""), L(o, true), f.append("pow", "NoPowUntilSvelte5ErrorFixed");
    const b = await fetch(t, { method: "POST", headers: { "Content-type": "application/x-www-form-urlencoded" }, body: f });
    let w = await b.json();
    b.status === 200 ? de.set(w) : L(i, Object.values(w)[0], true), L(o, false);
  }
  var j = wt();
  ge((v) => {
    var f = mt();
    qe.title = "Login", l(v, f);
  });
  var p = m(j), g = m(p);
  at(g, { get action() {
    return t;
  }, onSubmit: n, children: (v, f) => {
    var b = ht(), w = U(b);
    _t(w, { id: "password", name: "password", autocomplete: "current-password", placeholder: "Password", title: "Valid Dashboard Password", required: true });
    var T = _(w, 2);
    ke(T, { type: "submit", level: 1, get isLoading() {
      return r(o);
    }, children: (d, H) => {
      E();
      var R = B("Login");
      l(d, R);
    }, $$slots: { default: true } });
    var s = _(T, 2);
    {
      var y = (d) => {
        var H = ft(), R = m(H, true);
        u(H), P(() => C(R, r(i))), l(d, H);
      };
      A(s, (d) => {
        r(i) && d(y);
      });
    }
    l(v, b);
  }, $$slots: { default: true } }), u(p), u(j), l(a, j), Q();
}
var Z = ((a) => (a.Table = "table", a.Index = "index", a.Trigger = "view", a.View = "trigger", a))(Z || {}), bt = k(" <br/>", 1), yt = k('<section class="svelte-y9kii"><h5 class="header"> <br/> </h5> <div class="sql font-mono svelte-y9kii"></div></section>');
function kt(a, e) {
  O(e, true);
  let t = be(() => {
    var _a;
    return (_a = e.table.sql) == null ? void 0 : _a.split(`
`);
  });
  var i = yt(), o = m(i), n = m(o, true), j = _(n, 2);
  u(o);
  var p = _(o, 2);
  xe(p, 21, () => r(t), Fe, (g, v) => {
    E();
    var f = bt(), b = U(f, true);
    E(), P(() => C(b, r(v))), l(g, f);
  }), u(p), u(i), P(() => {
    C(n, e.table.name), C(j, ` ${e.table.typ ?? ""}: ${e.table.tbl_name ?? ""}`);
  }), l(a, i), Q();
}
function he(a, e, t) {
  e(t.view);
}
var pt = k('<div role="button" tabindex="0"> </div>');
function ae(a, e) {
  O(e, true);
  let t = S(e, "viewSelected", 15);
  var i = pt();
  i.__click = [he, t, e], i.__keydown = [he, t, e];
  var o = m(i, true);
  u(i), P(() => {
    Se(i, 1, Pe(t() === e.view ? "selected" : ""), "svelte-13ofdkm"), C(o, e.view);
  }), l(a, i), Q();
}
ue(["click", "keydown"]);
var xt = ce('<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M19.5 14.25v-2.625a3.375 3.375 0 00-3.375-3.375h-1.5A1.125 1.125 0 0113.5 7.125v-1.5a3.375 3.375 0 00-3.375-3.375H8.25m0 12.75h7.5m-7.5 3H12M10.5 2.25H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 00-9-9z"></path></svg>');
function St(a, e) {
  let t = S(e, "opacity", 8, 0.9), i = S(e, "width", 8, "1.5rem");
  var o = xt();
  x(o, "stroke-width", 2), P(() => {
    x(o, "width", i()), x(o, "opacity", t());
  }), l(a, o);
}
var Pt = k('<div class="err"> </div>'), jt = (a, e, t) => e(r(t).name), Mt = (a, e, t) => e(r(t).name), Lt = (a, e, t) => e(r(t).name), Ct = (a, e, t) => e(r(t).name), Tt = k('<div role="button" tabindex="0" class="btn svelte-trjj59"><!></div>'), qt = k('<div role="button" tabindex="0"><div> </div> <!></div>'), It = k('<!> <div class="selector svelte-trjj59"><!> <!> <!> <!></div> <div class="tables svelte-trjj59"><!> <!></div>', 1);
function At(a, e) {
  O(e, true);
  let t = I(le([])), i = I(void 0), o = I(le(Z.Table)), n = I(void 0);
  we(() => {
    j(r(o));
  });
  async function j(h) {
    let c = await je(`/tables/${h}`);
    c.status === 200 ? L(t, await c.json(), true) : L(n, await c.json(), true);
  }
  function p(h) {
    L(i, r(t).filter((c) => c.name === h)[0], true);
  }
  function g(h) {
    let c = { id: `${h}_${Qe(4)}`, query: `${Ye}
PRAGMA table_info(${h})` };
    Ge.push(c), p(h);
  }
  var v = It(), f = U(v);
  {
    var b = (h) => {
      var c = Pt(), G = m(c, true);
      u(c), P(() => C(G, r(n))), l(h, c);
    };
    A(f, (h) => {
      r(n) && h(b);
    });
  }
  var w = _(f, 2), T = m(w);
  ae(T, { get view() {
    return Z.Table;
  }, get viewSelected() {
    return r(o);
  }, set viewSelected(h) {
    L(o, h, true);
  } });
  var s = _(T, 2);
  ae(s, { get view() {
    return Z.Index;
  }, get viewSelected() {
    return r(o);
  }, set viewSelected(h) {
    L(o, h, true);
  } });
  var y = _(s, 2);
  ae(y, { get view() {
    return Z.Trigger;
  }, get viewSelected() {
    return r(o);
  }, set viewSelected(h) {
    L(o, h, true);
  } });
  var d = _(y, 2);
  ae(d, { get view() {
    return Z.View;
  }, get viewSelected() {
    return r(o);
  }, set viewSelected(h) {
    L(o, h, true);
  } }), u(w);
  var H = _(w, 2), R = m(H);
  Oe(R, { resizeBottom: true, initialHeightPx: window ? window.innerHeight - 400 : 600, minHeightPx: 120, children: (h, c) => {
    var G = Ie(), $ = U(G);
    xe($, 17, () => r(t), (X) => X.name, (X, z) => {
      var V = qt();
      V.__click = [jt, p, z], V.__keydown = [Mt, p, z];
      var J = m(V), re = m(J, true);
      u(J);
      var oe = _(J, 2);
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
      }), l(X, V);
    }), l(h, G);
  }, $$slots: { default: true } });
  var K = _(R, 2);
  {
    var W = (h) => {
      kt(h, { get table() {
        return r(i);
      } });
    };
    A(K, (h) => {
      r(i) && h(W);
    });
  }
  u(H), l(a, v), Q();
}
ue(["click", "keydown"]);
var Vt = k('<div class="metric svelte-1ktnipf"><div class="label font-label svelte-1ktnipf"> </div> <div class="font-mono"><!></div></div>');
function F(a, e) {
  var t = Vt(), i = m(t), o = m(i, true);
  u(i);
  var n = _(i, 2), j = m(n);
  ve(j, () => e.children), u(n), u(t), P(() => C(o, e.label)), l(a, t);
}
var Et = k('<b>Metrics</b> <div class="space svelte-12lemcq"></div> <!> <!> <!> <!> <!> <!> <!> <!>', 1);
function Ht(a, e) {
  O(e, true);
  let t = I(void 0), i = be(() => {
    var _a;
    return (_a = r(t)) == null ? void 0 : _a.membership_config.membership.configs.join(", ");
  });
  setInterval(() => {
    o();
  }, 1e4), ye(() => {
    o();
  });
  async function o() {
    let s = await je("/metrics");
    s.status === 200 ? L(t, await s.json(), true) : console.error(await s.json());
  }
  var n = Et(), j = _(U(n), 4);
  F(j, { label: "This Node", children: (s, y) => {
    E();
    var d = B();
    P(() => {
      var _a, _b;
      return C(d, `${((_a = r(t)) == null ? void 0 : _a.id) ?? ""}
    ${((_b = r(t)) == null ? void 0 : _b.state) ?? ""}`);
    }), l(s, d);
  } });
  var p = _(j, 2);
  F(p, { label: "Current Leader", children: (s, y) => {
    E();
    var d = B();
    P(() => {
      var _a;
      return C(d, (_a = r(t)) == null ? void 0 : _a.current_leader);
    }), l(s, d);
  } });
  var g = _(p, 2);
  F(g, { label: "Vote Leader", children: (s, y) => {
    E();
    var d = B();
    P(() => {
      var _a;
      return C(d, (_a = r(t)) == null ? void 0 : _a.vote.leader_id.node_id);
    }), l(s, d);
  } });
  var v = _(g, 2);
  F(v, { label: "Last Log Index", children: (s, y) => {
    E();
    var d = B();
    P(() => {
      var _a;
      return C(d, (_a = r(t)) == null ? void 0 : _a.last_log_index);
    }), l(s, d);
  } });
  var f = _(v, 2);
  F(f, { label: "Last Applied Log", children: (s, y) => {
    E();
    var d = B();
    P(() => {
      var _a, _b, _c, _d, _e2, _f;
      return C(d, `${((_b = (_a = r(t)) == null ? void 0 : _a.last_applied) == null ? void 0 : _b.leader_id.node_id) ?? ""}
    -
    ${((_d = (_c = r(t)) == null ? void 0 : _c.last_applied) == null ? void 0 : _d.leader_id.term) ?? ""}
    -
    ${((_f = (_e2 = r(t)) == null ? void 0 : _e2.last_applied) == null ? void 0 : _f.index) ?? ""}`);
    }), l(s, d);
  } });
  var b = _(f, 2);
  F(b, { label: "Last Snapshot", children: (s, y) => {
    E();
    var d = B();
    P(() => {
      var _a, _b, _c, _d;
      return C(d, `${((_b = (_a = r(t)) == null ? void 0 : _a.snapshot) == null ? void 0 : _b.leader_id) ?? ""}
    -
    ${((_d = (_c = r(t)) == null ? void 0 : _c.snapshot) == null ? void 0 : _d.index) ?? ""}`);
    }), l(s, d);
  } });
  var w = _(b, 2);
  F(w, { label: "Members", children: (s, y) => {
    E();
    var d = B();
    P(() => C(d, r(i))), l(s, d);
  } });
  var T = _(w, 2);
  F(T, { label: "Millis Quorum Ack", children: (s, y) => {
    E();
    var d = B();
    P(() => {
      var _a;
      return C(d, (_a = r(t)) == null ? void 0 : _a.millis_since_quorum_ack);
    }), l(s, d);
  } }), l(a, n), Q();
}
var Rt = k('<aside class="svelte-154rhoy"><!></aside>');
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
    L(e, t, true);
  } };
};
var Ft = k('<meta name="robots" content="noindex nofollow"/>'), Ot = k('<nav class="svelte-vv6eq"><!></nav> <main class="svelte-vv6eq"><div class="inner svelte-vv6eq"><!></div></main> <!>', 1), Qt = k("<!> <!>", 1);
function Nt(a, e) {
  O(e, true);
  let t = I(void 0), i = I(false);
  Bt("queries", [Ke]), de.subscribe((v) => {
    L(t, v, true);
  }), ye(async () => {
    let v = await fetch(`${pe}/session`);
    v.status === 200 && de.set(await v.json()), L(i, true);
  });
  var o = Qt();
  ge((v) => {
    var f = Ft();
    l(v, f);
  });
  var n = U(o);
  {
    var j = (v) => {
      var f = Ot(), b = U(f), w = m(b);
      At(w, {}), u(b);
      var T = _(b, 2), s = m(T), y = m(s);
      ve(y, () => e.children), u(s), u(T);
      var d = _(T, 2);
      zt(d), l(v, f);
    }, p = (v, f) => {
      {
        var b = (w) => {
          gt(w, {});
        };
        A(v, (w) => {
          r(i) && w(b);
        }, f);
      }
    };
    A(n, (v) => {
      r(t) ? v(j) : v(p, false);
    });
  }
  var g = _(n, 2);
  et(g), l(a, o), Q();
}
export {
  Nt as component,
  Jt as universal
};
