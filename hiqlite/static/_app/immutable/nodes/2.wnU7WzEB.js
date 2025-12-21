import { f as m, a as r, c as te, k as gt, j as we, e as Ae, s as ve, t as at } from "../chunks/U6xNcRH3.js";
import "../chunks/BUpFzQn_.js";
import { K as qt, be as Ht, as as Wt, p as _e, k as d, l as i, t as C, g as t, aa as oe, j as be, a as de, a8 as q, i as G, s as j, T as y, Y as ne, b as Ge, M as rt, am as nt, J as Ft, ba as Qt } from "../chunks/TbIIo73h.js";
import { s as Ie, a as c, k as ke, n as Xe, B as xe, r as Lt, b as Me, d as Nt, i as pe, j as Te, p as Vt, q as Yt, u as Zt, I as Kt, o as St, v as Gt, R as Xt, l as Jt, Q as re, w as $t, D as ea, f as ta } from "../chunks/WsmpKY91.js";
import { p as b, s as aa, a as ra, i as H, b as Ne } from "../chunks/Cn5KZez5.js";
import { s as na } from "../chunks/reTy-8zH.js";
function oa(g, e, a, _ = a) {
  e.addEventListener("input", () => {
    _(e[g]);
  }), qt(() => {
    var l = a();
    if (e[g] !== l) if (l == null) {
      var s = e[g];
      _(s);
    } else e[g] = l + "";
  });
}
function la(g, e) {
  Ht(window, ["resize"], () => Wt(() => e(window[g])));
}
const ia = () => {
  const g = na;
  return { page: { subscribe: g.page.subscribe }, navigating: { subscribe: g.navigating.subscribe }, updated: g.updated };
}, sa = { subscribe(g) {
  return ia().page.subscribe(g);
} };
var va = m('<span class="font-label"><a><!></a></span>');
function Pt(g, e) {
  _e(e, true);
  const a = () => ra(sa, "$page", _), [_, l] = aa();
  let s = b(e, "selectedStep", 3, false), x = b(e, "hideUnderline", 3, false), k = oe(() => {
    if (s()) return "step";
    if (a().route.id === e.href.split("?")[0]) return "page";
  });
  var p = va(), u = d(p);
  let E;
  var n = d(u);
  Ie(n, () => e.children), i(u), i(p), C(() => {
    c(u, "href", e.href), c(u, "target", e.target), c(u, "aria-current", t(k)), E = ke(u, 1, "svelte-1ltkamb", null, E, { hideUnderline: x() });
  }), r(g, p), be(), l();
}
var da = m('<!> <div class="popover svelte-okkiwd" popover="auto"><div class="inner fade-in svelte-okkiwd"><!></div></div>', 1);
function ot(g, e) {
  _e(e, true);
  let a = b(e, "ref", 15), _ = b(e, "roleButton", 3, "button"), l = b(e, "offsetLeft", 3, "0px"), s = b(e, "offsetTop", 3, "0px"), x = b(e, "close", 15);
  const k = Xe(8), p = Xe(8);
  let u = q(void 0), E = q(false);
  de(() => {
    x(f);
  });
  function n() {
    if (a() && t(u)) if (e.absolute) t(u).style.top = s(), t(u).style.left = l();
    else {
      let B = a().getBoundingClientRect();
      t(u).style.top = `calc(${B.bottom + window.scrollY}px + ${s()})`, t(u).style.left = `calc(${B.left + window.scrollX}px + ${l()})`;
    }
    else console.error("button and popover ref missing");
  }
  function f() {
    var _a2;
    (_a2 = t(u)) == null ? void 0 : _a2.hidePopover();
  }
  function D(B) {
    var _a2;
    let X = B.newState;
    y(E, X === "open"), (_a2 = e.onToggle) == null ? void 0 : _a2.call(e, X);
  }
  var L = da(), T = G(L);
  xe(T, { get role() {
    return _();
  }, get id() {
    return k;
  }, get ariaControls() {
    return p;
  }, get popovertarget() {
    return p;
  }, onclick: n, get invisible() {
    return e.btnInvisible;
  }, get isDisabled() {
    return e.btnDisabled;
  }, get onLeft() {
    return e.onLeft;
  }, get onRight() {
    return e.onRight;
  }, get onUp() {
    return e.onUp;
  }, get onDown() {
    return e.onDown;
  }, get ref() {
    return a();
  }, set ref(B) {
    a(B);
  }, children: (B, X) => {
    var S = te(), I = G(S);
    Ie(I, () => e.button), r(B, S);
  }, $$slots: { default: true } });
  var R = j(T, 2), W = d(R), M = d(W);
  {
    var V = (B) => {
      var X = te(), S = G(X);
      {
        var I = (o) => {
          var w = te(), z = G(w);
          Ie(z, () => e.children), r(o, w);
        };
        H(S, (o) => {
          t(E) && o(I);
        });
      }
      r(B, X);
    }, U = (B) => {
      var X = te(), S = G(X);
      Ie(S, () => e.children), r(B, X);
    };
    H(M, (B) => {
      e.lazy ? B(V) : B(U, false);
    });
  }
  i(W), i(R), Ne(R, (B) => y(u, B), () => t(u)), C(() => {
    c(R, "id", p), c(R, "aria-label", e.ariaLabel), c(R, "aria-labelledby", k);
  }), gt("toggle", R, D), r(g, L), be();
}
var ca = we('<svg fill="none" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M12 9.75L14.25 12m0 0l2.25 2.25M14.25 12l2.25-2.25M14.25 12L12 14.25m-2.58 4.92l-6.375-6.375a1.125 1.125 0 010-1.59L9.42 4.83c.211-.211.498-.33.796-.33H19.5a2.25 2.25 0 012.25 2.25v10.5a2.25 2.25 0 01-2.25 2.25h-9.284c-.298 0-.585-.119-.796-.33z"></path></svg>');
function ua(g, e) {
  let a = b(e, "color", 8, "currentColor"), _ = b(e, "opacity", 8, 0.9), l = b(e, "width", 8, "1.5rem");
  var s = ca();
  c(s, "stroke-width", 2), C(() => {
    c(s, "stroke", a()), c(s, "width", l()), c(s, "opacity", _());
  }), r(g, s);
}
var fa = we('<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M21 21l-5.197-5.197m0 0A7.5 7.5 0 105.196 5.196a7.5 7.5 0 0010.607 10.607z"></path></svg>');
function ha(g, e) {
  let a = b(e, "opacity", 8, 0.9), _ = b(e, "width", 8, "1.5rem");
  var l = fa();
  c(l, "stroke-width", 2), C(() => {
    c(l, "width", _()), c(l, "opacity", a());
  }), r(g, l);
}
var ga = m('<div class="options svelte-yyldap"><!></div>'), _a = m("<option></option>"), ba = m('<datalist class="absolute svelte-yyldap"></datalist>'), wa = m('<div class="magnify svelte-yyldap"><!></div>'), ma = m('<div class="btnSearch svelte-yyldap"><!></div>'), ya = m('<search class="flex container svelte-yyldap"><!> <input type="search" autocomplete="off" aria-label="Search" placeholder="Search" class="svelte-yyldap"/> <!> <div class="relative"><div class="absolute btnDelete svelte-yyldap"><!></div></div> <!></search>');
function ka(g, e) {
  _e(e, true);
  let a = b(e, "value", 15, ""), _ = b(e, "option", 15), l = b(e, "focus", 15), s = b(e, "width", 3, "100%");
  const x = Xe(8), k = Xe(8);
  let p = q(void 0), u = oe(() => e.datalist && e.datalist.length > 0 ? k : void 0);
  de(() => {
    l(f);
  });
  function E(o) {
    var _a2, _b, _c;
    switch (o.code) {
      case "Enter":
        n();
        break;
      case "Tab":
        (_a2 = e.onTab) == null ? void 0 : _a2.call(e, a());
        break;
      case "ArrowUp":
        (_b = e.onUp) == null ? void 0 : _b.call(e, a());
        break;
      case "ArrowDown":
        (_c = e.onDown) == null ? void 0 : _c.call(e, a());
        break;
    }
  }
  function n() {
    var _a2;
    (_a2 = e.onSearch) == null ? void 0 : _a2.call(e, a());
  }
  function f() {
    var _a2;
    (_a2 = t(p)) == null ? void 0 : _a2.focus();
  }
  var D = ya();
  let L;
  var T = d(D);
  {
    var R = (o) => {
      var w = ga(), z = d(w);
      Tt(z, { ariaLabel: "Search Options", get options() {
        return e.options;
      }, borderless: true, get value() {
        return _();
      }, set value(F) {
        _(F);
      } }), i(w), r(o, w);
    };
    H(T, (o) => {
      e.options && o(R);
    });
  }
  var W = j(T, 2);
  Lt(W), W.__keydown = E, Ne(W, (o) => y(p, o), () => t(p));
  var M = j(W, 2);
  {
    var V = (o) => {
      var w = ba();
      pe(w, 21, () => e.datalist, Te, (z, F, Y, Z) => {
        var le = _a(), ce = {};
        C(() => {
          ce !== (ce = t(F)) && (le.value = (le.__value = t(F)) ?? "");
        }), r(z, le);
      }), i(w), C(() => c(w, "id", k)), r(o, w);
    };
    H(M, (o) => {
      e.datalist && o(V);
    });
  }
  var U = j(M, 2), B = d(U), X = d(B);
  xe(X, { ariaLabel: "Delete Search Input", invisible: true, onclick: () => a(""), children: (o, w) => {
    ua(o, { color: "hsl(var(--bg-high))", width: 24 });
  }, $$slots: { default: true } }), i(B), i(U);
  var S = j(U, 2);
  {
    var I = (o) => {
      var w = ma(), z = d(w);
      xe(z, { ariaLabel: "Search", invisible: true, onclick: n, children: (F, Y) => {
        var Z = wa(), le = d(Z);
        ha(le, {}), i(Z), r(F, Z);
      }, $$slots: { default: true } }), i(w), r(o, w);
    };
    H(S, (o) => {
      e.onSearch && o(I);
    });
  }
  i(D), C(() => {
    L = Me(D, "", L, { border: e.borderless ? void 0 : "1px solid hsl(var(--bg-high))", width: s() }), c(W, "id", x), c(W, "list", t(u));
  }), gt("focus", W, () => {
    var _a2;
    return (_a2 = e.onFocus) == null ? void 0 : _a2.call(e);
  }), Nt(W, a), r(g, D), be();
}
Ae(["keydown"]);
var pa = we('<svg fill="none" viewBox="0 0 24 24" color="currentColor" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M8.25 4.5l7.5 7.5-7.5 7.5"></path></svg>');
function ht(g, e) {
  let a = b(e, "opacity", 8, 0.9), _ = b(e, "width", 8, "1.5rem");
  var l = pa();
  c(l, "stroke-width", 2), C(() => {
    c(l, "width", _()), c(l, "opacity", a());
  }), r(g, l);
}
var xa = m('<div class="btn svelte-fsrm4y"> <div class="chevron svelte-fsrm4y"><!></div></div>'), Ca = m('<div class="optPopover svelte-fsrm4y"> </div>'), Sa = m('<div role="listbox" tabindex="0" class="popover svelte-fsrm4y"><!> <div class="popoverOptions svelte-fsrm4y"></div></div>'), Pa = m('<option class="opt svelte-fsrm4y"> </option>'), La = m("<select></select>");
function Tt(g, e) {
  _e(e, true);
  let a = b(e, "ref", 15), _ = b(e, "options", 19, () => []), l = b(e, "value", 15), s = b(e, "asPopover", 3, true), x = b(e, "borderless", 3, false), k = b(e, "withSearch", 3, false), p = b(e, "fallbackOptions", 3, false), u = q(void 0), E = q(ne(p() ? false : s())), n = q(void 0), f = q(ne(k() ? -1 : 0)), D = q(void 0), L = q(""), T = oe(() => {
    if (!k()) return _();
    if (typeof l() == "string") return _().filter((w) => w.toLowerCase().includes(t(L).toLowerCase()));
    let o = Number.parseInt(t(L)) || l();
    return _().filter((w) => w === o);
  });
  de(() => {
    t(E) !== s() && y(E, s());
  }), de(() => {
    var _a2, _b;
    if (t(f) === -1 && ((_a2 = t(u)) == null ? void 0 : _a2.scrollTo({ top: 0, behavior: "smooth" })), k()) {
      if (t(f) < 0 || t(f) > t(T).length - 1) {
        y(f, -1), (_b = t(D)) == null ? void 0 : _b();
        return;
      }
    } else t(f) < 0 ? y(f, t(T).length - 1) : t(f) > t(T).length - 1 && y(f, 0), R();
  });
  function R() {
    if (t(u)) {
      let o = t(u).getElementsByTagName("button")[t(f)];
      o.scrollIntoView({ behavior: "smooth", block: "center" }), o.focus();
    } else console.error("refOptions is undefined");
  }
  function W(o) {
    var _a2;
    o === "open" && (k() ? (y(f, -1), (_a2 = t(D)) == null ? void 0 : _a2()) : (y(f, _().findIndex((w) => w === l()) || 0, true), R()));
  }
  function M(o) {
    let w = o.code;
    w === "ArrowDown" ? (o.preventDefault(), V() && y(f, t(f) + 1)) : w === "ArrowUp" ? (o.preventDefault(), V() && y(f, t(f) - 1)) : w === "Enter" && t(f) > -1 ? U(t(T)[t(f)]) : w === "Enter" && t(f) === -1 && t(T).length === 1 && U(t(T)[0]);
  }
  function V() {
    return t(T).length > 0 ? true : (y(f, -1), false);
  }
  function U(o) {
    l(o), y(L, ""), setTimeout(() => {
      var _a2;
      (_a2 = t(n)) == null ? void 0 : _a2();
    }, 20);
  }
  var B = te(), X = G(B);
  {
    var S = (o) => {
      ot(o, { get ariaLabel() {
        return e.ariaLabel;
      }, roleButton: "combobox", btnInvisible: true, get offsetTop() {
        return e.offsetTop;
      }, get offsetLeft() {
        return e.offsetLeft;
      }, onToggle: W, get onLeft() {
        return e.onLeft;
      }, get onRight() {
        return e.onRight;
      }, get onUp() {
        return e.onUp;
      }, get onDown() {
        return e.onDown;
      }, get ref() {
        return a();
      }, set ref(z) {
        a(z);
      }, get close() {
        return t(n);
      }, set close(z) {
        y(n, z, true);
      }, button: (z) => {
        var F = xa(), Y = d(F), Z = j(Y), le = d(Z);
        ht(le, { width: 14 }), i(Z), i(F), C(() => {
          c(F, "data-border", !x()), ve(Y, `${l() ?? ""} `);
        }), r(z, F);
      }, children: (z, F) => {
        var Y = Sa();
        Y.__keydown = M;
        let Z;
        var le = d(Y);
        {
          var ce = (ue) => {
            ka(ue, { onFocus: () => y(f, -1), get value() {
              return t(L);
            }, set value(me) {
              y(L, me, true);
            }, get focus() {
              return t(D);
            }, set focus(me) {
              y(D, me, true);
            } });
          };
          H(le, (ue) => {
            k() && ue(ce);
          });
        }
        var Ce = j(le, 2);
        pe(Ce, 21, () => t(T), Te, (ue, me, Ve) => {
          xe(ue, { invisible: true, invisibleOutline: true, onclick: () => U(t(me)), children: (Ye, lt) => {
            var De = Ca(), it = d(De, true);
            i(De), C(() => {
              c(De, "aria-selected", l() === t(me)), c(De, "data-focus", t(f) === Ve), ve(it, t(me));
            }), r(Ye, De);
          }, $$slots: { default: true } });
        }), i(Ce), Ne(Ce, (ue) => y(u, ue), () => t(u)), i(Y), C(() => Z = Me(Y, "", Z, { "max-height": e.maxHeight })), r(z, Y);
      }, $$slots: { button: true, default: true } });
    }, I = (o) => {
      var w = La();
      let z;
      pe(w, 21, () => t(T), Te, (F, Y) => {
        var Z = Pa(), le = d(Z, true);
        i(Z);
        var ce = {};
        C(() => {
          Vt(Z, l() === t(Y)), ve(le, t(Y)), ce !== (ce = t(Y)) && (Z.value = (Z.__value = t(Y)) ?? "");
        }), r(F, Z);
      }), i(w), C(() => {
        c(w, "name", e.name), c(w, "aria-label", e.ariaLabel), z = ke(w, 1, "svelte-fsrm4y", null, z, { borderless: x() });
      }), Yt(w, l), r(o, w);
    };
    H(X, (o) => {
      t(E) ? o(S) : o(I, false);
    });
  }
  r(g, B), be();
}
Ae(["keydown"]);
var Ta = m('<div class="link noselect svelte-14nrfpk"> </div>'), Ia = m('<li class="svelte-14nrfpk"><!></li>'), Da = m('<nav aria-label="Pagination" class="svelte-14nrfpk"><ul class="svelte-14nrfpk"></ul></nav>'), Ba = m('<div class="flex gap-10 svelte-14nrfpk"><div class="flex gap-05 chunkSize noselect svelte-14nrfpk"><div class="svelte-14nrfpk">Entries</div> <div class="svelte-14nrfpk"><!></div></div> <div class="font-label total svelte-14nrfpk"> </div></div>'), Oa = m('<div class="iconLeft svelte-14nrfpk" aria-label="Go to previous page"><!></div>'), za = m('<div class="iconRight svelte-14nrfpk" aria-label="Go to next page"><!></div>'), Ra = m('<div class="container svelte-14nrfpk"><!> <!> <!> <!></div>');
function Ma(g, e) {
  _e(e, true);
  const a = (S) => {
    var I = Da(), o = d(I);
    pe(o, 21, () => t(f), Te, (w, z) => {
      var F = Ia(), Y = d(F);
      xe(Y, { invisible: true, onclick: () => T(t(z)), onLeft: D, onRight: L, children: (Z, le) => {
        var ce = Ta(), Ce = d(ce, true);
        i(ce), C(() => ve(Ce, t(z))), r(Z, ce);
      }, $$slots: { default: true } }), i(F), C(() => {
        c(F, "aria-label", `go to page number: ${t(z)}`), c(F, "aria-current", x() === t(z) ? "step" : void 0);
      }), r(w, F);
    }), i(o), i(I), r(S, I);
  }, _ = (S) => {
    var I = Ba(), o = d(I), w = j(d(o), 2), z = d(w);
    Tt(z, { ariaLabel: "Page Count", get options() {
      return l;
    }, offsetTop: "-17rem", borderless: true, get value() {
      return k();
    }, set value(Z) {
      k(Z);
    } }), i(w), i(o);
    var F = j(o, 2), Y = d(F);
    i(F), i(I), C(() => ve(Y, `Total: ${e.items.length ?? ""}`)), r(S, I);
  }, l = [5, 7, 10, 15, 20, 30, 50, 100];
  let s = b(e, "itemsPaginated", 15), x = b(e, "page", 15, 1), k = b(e, "pageSize", 31, () => ne(l[0])), p = b(e, "compact", 3, false);
  const u = 16;
  let E = Ge(() => k()), n = q(ne([])), f = q(ne([]));
  de(() => {
    k() !== E && (E = Ge(() => k()), x(1));
  }), de(() => {
    let S = [];
    for (let I = 0; I < e.items.length; I += k()) {
      const o = e.items.slice(I, I + k());
      S.push(o);
    }
    y(n, S, true), s(S[x() - 1]);
  }), de(() => {
    R();
  });
  function D() {
    x() > 1 && T(x() - 1);
  }
  function L() {
    x() < t(n).length && T(x() + 1);
  }
  function T(S) {
    x(S), R();
  }
  function R() {
    let S = [], I = Math.floor(k() / 2);
    if (t(n).length <= k()) for (let o = 1; o <= t(n).length; o++) S.push(o);
    else if (x() <= I) for (let o = 1; o <= k(); o++) S.push(o);
    else if (x() > t(n).length - I - 1) for (let o = t(n).length - k(); o <= t(n).length - 1; o++) S.push(o + 1);
    else for (let o = x() - I; o < x() - I + k(); o++) S.push(o);
    y(f, S, true);
  }
  var W = Ra(), M = d(W);
  {
    let S = oe(() => x() === 1);
    xe(M, { onclick: D, invisible: true, get isDisabled() {
      return t(S);
    }, children: (I, o) => {
      var w = Oa(), z = d(w);
      ht(z, { width: u }), i(w), C(() => c(w, "data-disabled", x() === 1)), r(I, w);
    }, $$slots: { default: true } });
  }
  var V = j(M, 2);
  a(V);
  var U = j(V, 2);
  {
    let S = oe(() => x() === t(n).length);
    xe(U, { onclick: L, invisible: true, get isDisabled() {
      return t(S);
    }, children: (I, o) => {
      var w = za(), z = d(w);
      ht(z, { width: u }), i(w), C(() => c(w, "data-disabled", x() === t(n).length)), r(I, w);
    }, $$slots: { default: true } });
  }
  var B = j(U, 2);
  {
    var X = (S) => {
      _(S);
    };
    H(B, (S) => {
      p() || S(X);
    });
  }
  i(W), r(g, W), be();
}
var Aa = m('<label class="font-label noselect svelte-136uhkj"><input type="checkbox" class="svelte-136uhkj"/> <span class="svelte-136uhkj"><!></span></label>');
function Ke(g, e) {
  _e(e, true);
  let a = b(e, "checked", 15, false), _ = b(e, "ariaLabel", 3, ""), l = b(e, "borderColor", 3, "hsl(var(--bg-high))");
  function s() {
    a(!a());
  }
  function x(f) {
    console.log(f.code), f.code === "Enter" && s();
  }
  var k = Aa(), p = d(k);
  Lt(p), p.__click = s, p.__keydown = x;
  let u;
  var E = j(p, 2), n = d(E);
  Ie(n, () => e.children ?? rt), i(E), i(k), C(() => {
    c(p, "name", e.name), p.disabled = e.disabled, c(p, "aria-disabled", e.disabled), c(p, "aria-checked", a()), c(p, "aria-label", _()), u = Me(p, "", u, { "border-color": l() });
  }), Zt(p, a), r(g, k), be();
}
Ae(["click", "keydown"]);
var ja = we('<svg fill="none" viewBox="0 0 24 24" stroke="currentColor" color="hsl(var(--action))"><path stroke-linecap="round" stroke-linejoin="round" d="M4.5 12.75l6 6 9-13.5"></path></svg>'), Ea = we('<svg fill="none" viewBox="0 0 24 24" stroke="currentColor" color="hsl(var(--error))"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12"></path></svg>');
function Ua(g, e) {
  _e(e, true);
  const a = 20, _ = 0.9;
  var l = te(), s = G(l);
  {
    var x = (p) => {
      var u = ja();
      c(u, "stroke-width", 2), c(u, "width", a), c(u, "opacity", _), r(p, u);
    }, k = (p) => {
      var u = Ea();
      c(u, "stroke-width", 2), c(u, "width", a), c(u, "opacity", _), r(p, u);
    };
    H(s, (p) => {
      e.checked ? p(x) : p(k, false);
    });
  }
  r(g, l), be();
}
var qa = we('<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m19.5 8.25-7.5 7.5-7.5-7.5"></path></svg>');
function Ha(g, e) {
  let a = b(e, "color", 8, "currentColor"), _ = b(e, "opacity", 8, 0.9), l = b(e, "width", 8, "1.5rem");
  var s = qa();
  c(s, "stroke-width", 2), C(() => {
    c(s, "width", l()), c(s, "color", a()), c(s, "opacity", _());
  }), r(g, s);
}
var Wa = we('<svg fill="none" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M3 7.5 7.5 3m0 0L12 7.5M7.5 3v13.5m13.5 0L16.5 21m0 0L12 16.5m4.5 4.5V7.5"></path></svg>');
function Fa(g, e) {
  let a = b(e, "color", 8, "currentColor"), _ = b(e, "opacity", 8, 0.9), l = b(e, "width", 8, "1.5rem");
  var s = Wa();
  c(s, "stroke-width", 2), C(() => {
    c(s, "stroke", a()), c(s, "width", l()), c(s, "opacity", _());
  }), r(g, s);
}
var Qa = we(`<svg fill="none" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213
            1.281c.063.374.313.686.645.87.074.04.147.083.22.127.325.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 0
            1 1.37.49l1.296 2.247a1.125 1.125 0 0 1-.26 1.431l-1.003.827c-.293.241-.438.613-.43.992a7.723 7.723 0 0
            1 0 .255c-.008.378.137.75.43.991l1.004.827c.424.35.534.955.26 1.43l-1.298 2.247a1.125 1.125 0 0
            1-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.47 6.47 0 0
            1-.22.128c-.331.183-.581.495-.644.869l-.213 1.281c-.09.543-.56.94-1.11.94h-2.594c-.55 0-1.019-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52
            0 0 1-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 0 1-1.369-.49l-1.297-2.247a1.125
            1.125 0 0 1 .26-1.431l1.004-.827c.292-.24.437-.613.43-.991a6.932 6.932 0 0 1 0-.255c.007-.38-.138-.751-.43-.992l-1.004-.827a1.125 1.125
            0 0 1-.26-1.43l1.297-2.247a1.125 1.125 0 0 1 1.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.086.22-.128.332-.183.582-.495.644-.869l.214-1.28Z"></path><path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z"></path></svg>`);
function Na(g, e) {
  let a = b(e, "color", 8, "currentColor"), _ = b(e, "opacity", 8, 0.9), l = b(e, "width", 8, "1.5rem");
  var s = Qa();
  c(s, "stroke-width", 2), C(() => {
    c(s, "stroke", a()), c(s, "width", l()), c(s, "opacity", _());
  }), r(g, s);
}
async function Va(g) {
  var _a2;
  await ((_a2 = navigator == null ? void 0 : navigator.clipboard) == null ? void 0 : _a2.writeText(g));
}
var Ya = we(`<svg fill="none" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M6.75 12a.75.75 0 1 1-1.5 0 .75.75 0 0 1 1.5 0ZM12.75 12a.75.75 0 1 1-1.5 0 .75.75 0 0 1 1.5
            0ZM18.75 12a.75.75 0 1 1-1.5 0 .75.75 0 0 1 1.5 0Z"></path></svg>`);
function Za(g, e) {
  let a = b(e, "color", 8, "currentColor"), _ = b(e, "opacity", 8, 0.9), l = b(e, "width", 8, "1.5rem");
  var s = Ya();
  c(s, "stroke-width", 2), C(() => {
    c(s, "stroke", a()), c(s, "width", l()), c(s, "opacity", _());
  }), r(g, s);
}
var Ka = m('<span class="btnSelect svelte-14anyh0"><!></span>'), Ga = m('<th class="headerCheckbox svelte-14anyh0"><!> <!></th>'), Xa = m('<span class="iconOrder svelte-14anyh0"><!></span>'), Ja = m('<span class="orderText svelte-14anyh0"> </span> <!>', 1), $a = m('<span class="rawText svelte-14anyh0"> </span>'), er = m('<th class="svelte-14anyh0"><span class="flex-1 label svelte-14anyh0"><!></span> <span class="relative"><span role="none" class="absolute sizeable svelte-14anyh0"></span></span></th>'), tr = m('<th class="headerOptions svelte-14anyh0"><!></th>'), ar = m('<td class="checkbox svelte-14anyh0"><!></td>'), rr = m("<span> </span>"), nr = m("<span> </span>"), or = m("<span> </span>"), lr = m("<span><!></span>"), ir = m("<span> </span>"), sr = m("<span> </span>"), vr = m('<td class="svelte-14anyh0"><!></td>'), dr = m('<span class="btnOptions svelte-14anyh0"><!></span>'), cr = m('<td class="options svelte-14anyh0"><!></td>'), ur = m("<tr><!><!><!></tr>"), fr = m('<div class="eye svelte-14anyh0"><!></div>'), hr = m('<div class="columnSelect svelte-14anyh0"><!></div>'), gr = m('<div class="columnSelect svelte-14anyh0"><!></div>'), _r = m('<div class="columnSelect svelte-14anyh0"><!></div>'), br = m('<div class="columnsSelect svelte-14anyh0"><!> <!> <!></div>'), wr = m("<div></div>"), mr = m('<table class="svelte-14anyh0"><thead class="svelte-14anyh0"><tr class="svelte-14anyh0"><!><!><!></tr></thead><tbody class="svelte-14anyh0"><!></tbody><caption class="flex space-between svelte-14anyh0"><div class="flex"><!> <div class="caption svelte-14anyh0"><!></div> <!></div></caption></table>');
function yr(g, e) {
  _e(e, true);
  let a = b(e, "showColumns", 31, () => ne(Array(e.columns.length).fill(true))), _ = b(e, "paginationCompact", 3, false), l = b(e, "paginationPageSize", 3, 15), s = b(e, "selectInitHide", 3, false), x = b(e, "minWidthColPx", 3, 50);
  const k = "3rem", p = "2rem";
  let u = ne(w()), E = q(ne(Ge(() => u))), n = q(1), f = q(ne(Ge(() => l()))), D = q(false), L = q(ne(Array(e.rows.length).fill(false))), T = oe(() => t(L).find((v) => v === true)), R = q(void 0), W = q(ne([])), M = q("up"), V = q(ne([])), U = oe(() => e.paginationDisabled !== void 0 ? e.paginationDisabled : e.rows.length < l()), B = oe(() => e.paginationDisabled || t(U) ? e.rows.length : t(V) && t(V).length ? t(V).length : (t(n) != 1 && y(n, 1), 0)), X = ne(Array(Ge(() => u.length)).fill(void 0)), S = q(void 0), I = 0, o = q(void 0);
  de(() => {
    setTimeout(() => {
      for (let v = 1; v < u.length; v++) if (u[v] === "auto") {
        I = e.select ? v : v - 1;
        let h = X[v];
        h && Ce(h.getBoundingClientRect().width);
      }
    }, 150);
  }), de(() => {
    let v = Array(e.rows.length).fill(false);
    if (t(D)) {
      let h;
      t(n) === 1 ? h = 0 : h = (t(n) - 1) * t(f);
      let O = Math.min(t(n) * t(f), e.rows.length);
      for (let A = h; A < O; A++) v[A] = true;
    }
    y(L, v, true);
  }), de(() => {
    let v = e.paginationDisabled || t(U) ? e.rows.length : t(V).length || 0;
    y(W, Array(v).fill(() => console.error("un-initialized popover close option")), true);
  }), de(() => {
    let v = [];
    for (let h = 0; h < u.length; h++) a()[h] && v.push(u[h]);
    y(E, v, true);
  }), de(() => {
    if (e.highlight !== void 0 && t(S)) {
      let v = e.highlight;
      !t(U) && t(n) > 1 && (v = e.highlight - (t(n) - 1) * t(f)), y(o, v, true), setTimeout(() => {
        var _a2, _b;
        (_b = (_a2 = t(S)) == null ? void 0 : _a2.getElementsByClassName("highlight")[0]) == null ? void 0 : _b.scrollIntoView({ behavior: "smooth", block: "center" });
      }, 250);
    } else y(o, void 0);
  });
  function w() {
    let v = e.columns.map((O) => O.initialWidth), h = [...a()];
    return e.select && (v = [k, ...v], h = [!s(), ...h]), e.options && (v = [...v, p], h = [...h, true]), a(h), v;
  }
  function z() {
    return t(E).join(" ");
  }
  function F(v, h) {
    y(L, Array(e.rows.length).fill(false), true);
    let O = 1;
    t(M) === "up" ? (O = -1, y(M, "down")) : y(M, "up"), h === "string" ? e.rows.sort((A, Q) => A[v].content.localeCompare(Q[v].content) * O) : h === "number" && e.rows.sort((A, Q) => (A[v].content - Q[v].content) * O);
  }
  function Y(v) {
    return !t(U) && t(n) > 1 ? (t(n) - 1) * t(f) + v : v;
  }
  function Z(v) {
    I = v;
    let h = X[v];
    h ? (Ce(h.getBoundingClientRect().width), window.addEventListener("mousemove", ce), window.addEventListener("mouseup", le, { once: true })) : console.error("invalid ref from refCols in onMouseDown");
  }
  function le() {
    window.removeEventListener("mousemove", ce);
  }
  function ce(v) {
    let h = X[I];
    if (h) {
      let O = h.getBoundingClientRect().left, A = window.scrollX + v.x - O;
      Ce(A);
    } else console.error("invalid ref from refCols in onMove");
  }
  function Ce(v) {
    v = Math.ceil(v), v < x() && (v = x()), u[e.select ? I + 1 : I] = `${v}px`;
  }
  var ue = mr();
  let me;
  var Ve = d(ue), Ye = d(Ve);
  let lt;
  var De = d(Ye);
  {
    var it = (v) => {
      var h = Ga(), O = d(h);
      Ke(O, { ariaLabel: "Select All", borderColor: "hsla(var(--text), .4)", get checked() {
        return t(D);
      }, set checked(Q) {
        y(D, Q, true);
      } });
      var A = j(O, 2);
      {
        const Q = ($) => {
          var ee = Ka(), fe = d(ee);
          Ha(fe, { width: "1rem" }), i(ee), C(() => c(ee, "data-disabled", !t(T))), r($, ee);
        };
        let ie = oe(() => !t(T));
        ot(A, { ariaLabel: "Selected Options", get btnDisabled() {
          return t(ie);
        }, btnInvisible: true, get close() {
          return t(R);
        }, set close($) {
          y(R, $, true);
        }, button: Q, children: ($, ee) => {
          var fe = te(), ye = G(fe);
          Ie(ye, () => e.select, () => t(L), () => t(R)), r($, fe);
        }, $$slots: { button: true, default: true } });
      }
      i(h), r(v, h);
    };
    H(De, (v) => {
      e.select && a()[0] && v(it);
    });
  }
  var _t = j(De);
  pe(_t, 17, () => e.columns, Te, (v, h, O) => {
    var A = te(), Q = G(A);
    {
      var ie = ($) => {
        var ee = er(), fe = d(ee), ye = d(fe);
        {
          var se = (N) => {
            var J = Ja(), ae = G(J), Se = d(ae, true);
            i(ae);
            var ge = j(ae, 2);
            xe(ge, { invisible: true, onclick: () => F(O, t(h).orderType), children: (ze, yt) => {
              var Ze = Xa(), vt = d(Ze);
              Fa(vt, { width: "1rem" }), i(Ze), r(ze, Ze);
            }, $$slots: { default: true } }), C(() => ve(Se, t(h).content)), r(N, J);
          }, he = (N) => {
            var J = $a(), ae = d(J, true);
            i(J), C(() => ve(ae, t(h).content)), r(N, J);
          };
          H(ye, (N) => {
            t(h).orderType ? N(se) : N(he, false);
          });
        }
        i(fe);
        var K = j(fe, 2), P = d(K);
        P.__mousedown = () => Z(O), i(K), i(ee), Ne(ee, (N, J) => X[J] = N, (N) => X == null ? void 0 : X[N], () => [O]), r($, ee);
      };
      H(Q, ($) => {
        a()[e.select ? O + 1 : O] && $(ie);
      });
    }
    r(v, A);
  });
  var It = j(_t);
  {
    var Dt = (v) => {
      var h = tr(), O = d(h);
      Na(O, { width: "1.2rem" }), i(h), r(v, h);
    };
    H(It, (v) => {
      e.options && a()[a().length - 1] && v(Dt);
    });
  }
  i(Ye), i(Ve);
  var Je = j(Ve);
  {
    const v = (h, O = rt, A = rt) => {
      var Q = ur();
      let ie, $;
      var ee = d(Q);
      {
        var fe = (K) => {
          var P = ar(), N = d(P);
          Ke(N, { ariaLabel: "Select Row", get checked() {
            return t(L)[Y(A())];
          }, set checked(J) {
            t(L)[Y(A())] = J;
          } }), i(P), r(K, P);
        };
        H(ee, (K) => {
          e.select && a()[0] && K(fe);
        });
      }
      var ye = j(ee);
      pe(ye, 17, O, Te, (K, P, N) => {
        var J = te(), ae = G(J);
        {
          var Se = (ge) => {
            var ze = vr(), yt = d(ze);
            {
              var Ze = (je) => {
                {
                  let $e = oe(() => t(P).href || "");
                  Pt(je, { get href() {
                    return t($e);
                  }, children: (dt, kt) => {
                    var Ee = rr();
                    let Pe;
                    var Ue = d(Ee, true);
                    i(Ee), C(() => {
                      Pe = ke(Ee, 1, "linkText nowrap svelte-14anyh0", null, Pe, { muted: t(P).muted }), ve(Ue, t(P).content);
                    }), r(dt, Ee);
                  }, $$slots: { default: true } });
                }
              }, vt = (je) => {
                var $e = te(), dt = G($e);
                {
                  var kt = (Pe) => {
                    {
                      let Ue = oe(() => t(P).href || "");
                      Pt(Pe, { get href() {
                        return t(Ue);
                      }, target: "_blank", children: (ct, pt) => {
                        var qe = nr();
                        let Le;
                        var He = d(qe, true);
                        i(qe), C(() => {
                          Le = ke(qe, 1, "linkText nowrap svelte-14anyh0", null, Le, { muted: t(P).muted }), ve(He, t(P).content);
                        }), r(ct, qe);
                      }, $$slots: { default: true } });
                    }
                  }, Ee = (Pe) => {
                    var Ue = te(), ct = G(Ue);
                    {
                      var pt = (Le) => {
                        xe(Le, { invisible: true, onclick: () => Va(t(P).content.toString()), children: (He, xt) => {
                          var We = or();
                          let et;
                          var Be = d(We, true);
                          i(We), C(() => {
                            et = ke(We, 1, "copyToClip nowrap svelte-14anyh0", null, et, { muted: t(P).muted }), ve(Be, t(P).content);
                          }), r(He, We);
                        }, $$slots: { default: true } });
                      }, qe = (Le) => {
                        var He = te(), xt = G(He);
                        {
                          var We = (Be) => {
                            var Re = lr();
                            let tt;
                            var ut = d(Re);
                            Ua(ut, { get checked() {
                              return t(P).content;
                            } }), i(Re), C(() => tt = ke(Re, 1, "checkIcon nowrap svelte-14anyh0", null, tt, { muted: t(P).muted })), r(Be, Re);
                          }, et = (Be) => {
                            var Re = te(), tt = G(Re);
                            {
                              var ut = (Fe) => {
                                xe(Fe, { invisible: true, onclick: (Oe) => {
                                  var _a2, _b;
                                  return (_b = (_a2 = t(P)).onClick) == null ? void 0 : _b.call(_a2, Oe, Y(A()));
                                }, children: (Oe, ft) => {
                                  var Qe = ir();
                                  let Ct;
                                  var Ut = d(Qe, true);
                                  i(Qe), C(() => {
                                    Ct = ke(Qe, 1, "onclick nowrap svelte-14anyh0", null, Ct, { muted: t(P).muted }), ve(Ut, t(P).content);
                                  }), r(Oe, Qe);
                                }, $$slots: { default: true } });
                              }, Et = (Fe) => {
                                var Oe = sr();
                                let ft;
                                var Qe = d(Oe, true);
                                i(Oe), C(() => {
                                  ft = ke(Oe, 1, "rawText nowrap svelte-14anyh0", null, ft, { muted: t(P).muted }), ve(Qe, t(P).content);
                                }), r(Fe, Oe);
                              };
                              H(tt, (Fe) => {
                                t(P).onClick ? Fe(ut) : Fe(Et, false);
                              }, true);
                            }
                            r(Be, Re);
                          };
                          H(xt, (Be) => {
                            var _a2;
                            ((_a2 = e.columns[N]) == null ? void 0 : _a2.showAs) === "check" ? Be(We) : Be(et, false);
                          }, true);
                        }
                        r(Le, He);
                      };
                      H(ct, (Le) => {
                        var _a2;
                        ((_a2 = e.columns[N]) == null ? void 0 : _a2.showAs) === "copyToClip" ? Le(pt) : Le(qe, false);
                      }, true);
                    }
                    r(Pe, Ue);
                  };
                  H(dt, (Pe) => {
                    var _a2;
                    ((_a2 = e.columns[N]) == null ? void 0 : _a2.showAs) === "a_blank" ? Pe(kt) : Pe(Ee, false);
                  }, true);
                }
                r(je, $e);
              };
              H(yt, (je) => {
                var _a2;
                ((_a2 = e.columns[N]) == null ? void 0 : _a2.showAs) === "a" ? je(Ze) : je(vt, false);
              });
            }
            i(ze), r(ge, ze);
          };
          H(ae, (ge) => {
            a()[e.select ? N + 1 : N] && ge(Se);
          });
        }
        r(K, J);
      });
      var se = j(ye);
      {
        var he = (K) => {
          var P = cr(), N = d(P);
          ot(N, { ariaLabel: "Options", btnInvisible: true, get offsetLeft() {
            return e.offsetLeftOptions;
          }, get offsetTop() {
            return e.offsetTopOptions;
          }, get close() {
            return t(W)[A()];
          }, set close(ae) {
            t(W)[A()] = ae;
          }, button: (ae) => {
            var Se = dr(), ge = d(Se);
            Za(ge, {}), i(Se), r(ae, Se);
          }, children: (ae, Se) => {
            var ge = te(), ze = G(ge);
            Ie(ze, () => e.options, O, () => t(W)[A()]), r(ae, ge);
          }, $$slots: { button: true, default: true } }), i(P), r(K, P);
        };
        H(se, (K) => {
          e.options && a()[a().length - 1] && K(he);
        });
      }
      i(Q), C((K) => {
        ie = ke(Q, 1, "svelte-14anyh0", null, ie, { highlight: t(o) === A() }), $ = Me(Q, "", $, K);
      }, [() => ({ "grid-template-columns": z() })]), r(h, Q);
    };
    var Bt = d(Je);
    {
      var Ot = (h) => {
        var O = te(), A = G(O);
        {
          var Q = (ie) => {
            var $ = te(), ee = G($);
            pe(ee, 17, () => e.rows, Te, (fe, ye, se) => {
              v(fe, () => t(ye), () => se);
            }), r(ie, $);
          };
          H(A, (ie) => {
            t(L).length === e.rows.length && ie(Q);
          });
        }
        r(h, O);
      }, zt = (h) => {
        var O = te(), A = G(O);
        pe(A, 17, () => t(V), Te, (Q, ie, $) => {
          v(Q, () => t(ie), () => $);
        }), r(h, O);
      };
      H(Bt, (h) => {
        t(U) ? h(Ot) : h(zt, false);
      });
    }
    i(Je), Ne(Je, (h) => y(S, h), () => t(S));
  }
  var bt = j(Je), wt = d(bt), mt = d(wt);
  {
    const v = (O) => {
      var A = fr(), Q = d(A);
      Kt(Q, {}), i(A), r(O, A);
    };
    let h = oe(() => `-${u.length * 1.4 + 3}rem`);
    ot(mt, { ariaLabel: "Select Columns", get offsetTop() {
      return t(h);
    }, btnInvisible: true, button: v, children: (O, A) => {
      var Q = br(), ie = d(Q);
      {
        var $ = (se) => {
          var he = hr(), K = d(he);
          Ke(K, { ariaLabel: "Select Column: Select", get checked() {
            return a()[0];
          }, set checked(P) {
            a(a()[0] = P, true);
          }, children: (P, N) => {
            nt();
            var J = at("Select");
            r(P, J);
          }, $$slots: { default: true } }), i(he), r(se, he);
        };
        H(ie, (se) => {
          e.select && se($);
        });
      }
      var ee = j(ie, 2);
      pe(ee, 17, () => e.columns, Te, (se, he, K) => {
        var P = gr(), N = d(P);
        {
          let J = oe(() => `Select Column: ${t(he).content}`);
          Ke(N, { get ariaLabel() {
            return t(J);
          }, get checked() {
            return a()[e.select ? K + 1 : K];
          }, set checked(ae) {
            a(a()[e.select ? K + 1 : K] = ae, true);
          }, children: (ae, Se) => {
            nt();
            var ge = at();
            C(() => ve(ge, t(he).content)), r(ae, ge);
          }, $$slots: { default: true } });
        }
        i(P), r(se, P);
      });
      var fe = j(ee, 2);
      {
        var ye = (se) => {
          var he = _r(), K = d(he);
          Ke(K, { ariaLabel: "Select Column: Options", get checked() {
            return a()[a().length - 1];
          }, set checked(P) {
            a(a()[a().length - 1] = P, true);
          }, children: (P, N) => {
            nt();
            var J = at("Options");
            r(P, J);
          }, $$slots: { default: true } }), i(he), r(se, he);
        };
        H(fe, (se) => {
          e.options && se(ye);
        });
      }
      i(Q), r(O, Q);
    }, $$slots: { button: true, default: true } });
  }
  var st = j(mt, 2), Rt = d(st);
  Ie(Rt, () => e.caption ?? rt), i(st);
  var Mt = j(st, 2);
  {
    var At = (v) => {
      var h = wr();
      r(v, h);
    }, jt = (v) => {
      Ma(v, { get items() {
        return e.rows;
      }, get compact() {
        return _();
      }, get itemsPaginated() {
        return t(V);
      }, set itemsPaginated(h) {
        y(V, h, true);
      }, get page() {
        return t(n);
      }, set page(h) {
        y(n, h, true);
      }, get pageSize() {
        return t(f);
      }, set pageSize(h) {
        y(f, h, true);
      } });
    };
    H(Mt, (v) => {
      t(U) ? v(At) : v(jt, false);
    });
  }
  i(wt), i(bt), i(ue), C((v) => {
    c(ue, "aria-colcount", u.length), c(ue, "aria-rowcount", t(B)), me = Me(ue, "", me, { width: e.width, "max-width": e.maxWidth }), lt = Me(Ye, "", lt, v);
  }, [() => ({ "grid-template-columns": z() })]), r(g, ue), be();
}
Ae(["mousedown"]);
var kr = m("<p>no results</p>");
function pr(g, e) {
  _e(e, true);
  let a = q(ne([])), _ = q(ne([]));
  de(() => {
    let n = [], f = [];
    if (e.rows.length > 0) {
      for (let D of e.rows[0].columns) n.push({ content: D.name, initialWidth: "12rem", orderType: s(D.value) });
      for (let D of e.rows) {
        let L = [];
        for (let T of D.columns) L.push({ content: x(T.value) });
        f.push(L);
      }
    }
    y(a, n, true), y(_, f, true);
  });
  function l(n) {
    return [...new Uint8Array(n)].map((f) => f.toString(16).padStart(2, "0")).join("");
  }
  function s(n) {
    return n.hasOwnProperty("Integer") || n.hasOwnProperty("Real") ? "number" : "string";
  }
  function x(n) {
    return n.hasOwnProperty("Integer") ? n.Integer : n.hasOwnProperty("Real") ? n.Real : n.hasOwnProperty("Text") ? n.Text : n.hasOwnProperty("Blob") ? `x'${l(n.Blob)}'` : "NULL";
  }
  var k = te(), p = G(k);
  {
    var u = (n) => {
      yr(n, { get columns() {
        return t(a);
      }, paginationPageSize: 100, get rows() {
        return t(_);
      }, set rows(f) {
        y(_, f, true);
      } });
    }, E = (n) => {
      var f = kr();
      r(n, f);
    };
    H(p, (n) => {
      t(a).length > 0 && t(_).length > 0 ? n(u) : n(E, false);
    });
  }
  r(g, k), be();
}
var xr = m('<div role="textbox" tabindex="0" class="query svelte-13lgqmr" contenteditable=""></div>'), Cr = m('<div class="err"> </div>'), Sr = m('<!> <!> <div id="query-results" class="svelte-13lgqmr"><!></div>', 1);
function Pr(g, e) {
  _e(e, true);
  let a = b(e, "query", 7), _ = q(ne([])), l = q(""), s = q(void 0), x = q(void 0), k = oe(() => t(s) && t(x) ? `${t(s) - t(x)}px` : "100%");
  de(() => {
    a().query.startsWith(St) && (a().query = a().query.replace(`${St}
`, ""), u());
  });
  function p(M) {
    M.ctrlKey && M.code === "Enter" && u();
  }
  async function u() {
    y(_, [], true), y(l, "");
    let M = [];
    for (let B of a().query.split(/\r?\n/)) B.startsWith("--") || M.push(B);
    let V = M.join(`
`), U = await Gt("/query", V);
    if (U.status === 200) y(_, await U.json(), true);
    else {
      let B = await U.json();
      y(l, Object.values(B)[0], true);
    }
  }
  async function E(M) {
    y(x, M, true);
  }
  var n = Sr(), f = G(n);
  Xt(f, { resizeBottom: true, minHeightPx: 100, initialHeightPx: 300, onResizeBottom: E, children: (M, V) => {
    var U = xr();
    U.__keydown = p, oa("innerText", U, () => a().query, (B) => a().query = B), r(M, U);
  }, $$slots: { default: true } });
  var D = j(f, 2);
  {
    var L = (M) => {
      var V = Cr(), U = d(V, true);
      i(V), C(() => ve(U, t(l))), r(M, V);
    };
    H(D, (M) => {
      t(l) && M(L);
    });
  }
  var T = j(D, 2);
  let R;
  var W = d(T);
  pr(W, { get rows() {
    return t(_);
  }, set rows(M) {
    y(_, M, true);
  } }), i(T), C(() => R = Me(T, "", R, { height: t(k), "max-height": t(k) })), la("innerHeight", (M) => y(s, M, true)), r(g, n), be();
}
Ae(["keydown"]);
var Lr = we('<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12"></path></svg>');
function Tr(g, e) {
  let a = b(e, "color", 8, "var(--col-err)"), _ = b(e, "opacity", 8, 0.9), l = b(e, "width", 8, "1.5rem");
  var s = Lr();
  c(s, "stroke-width", 2), C(() => {
    c(s, "width", l()), c(s, "color", a()), c(s, "opacity", _());
  }), r(g, s);
}
var Ir = m('<div class="row svelte-4zjo8c"><div role="button" tabindex="0"><!></div> <div class="close svelte-4zjo8c"><div role="button" tabindex="0" class="close-inner svelte-4zjo8c"><!></div></div></div>');
function Dr(g, e) {
  _e(e, true);
  let a = b(e, "tab", 15), _ = b(e, "tabSelected", 15), l, s = oe(() => _() === a());
  function x(R) {
    t(s) ? R.code === "Enter" && (R.preventDefault(), k()) : p();
  }
  function k() {
    let R = l.innerText;
    a(R), _(R);
  }
  function p() {
    t(s) || _(a());
  }
  function u() {
    e.onClose(a());
  }
  var E = Ir(), n = d(E);
  n.__click = p, n.__keydown = x;
  var f = d(n);
  Ie(f, () => e.children), i(n), Ne(n, (R) => l = R, () => l);
  var D = j(n, 2), L = d(D);
  L.__click = u, L.__keydown = u;
  var T = d(L);
  Tr(T, { color: "hsl(var(--error))", width: "1.2rem" }), i(L), i(D), i(E), C(() => {
    ke(n, 1, Jt(t(s) ? "tab selected" : "tab"), "svelte-4zjo8c"), c(n, "contenteditable", t(s));
  }), gt("blur", n, k), r(g, E), be();
}
Ae(["click", "keydown"]);
var Br = we('<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15"></path></svg>');
function Or(g, e) {
  let a = b(e, "opacity", 8, 0.9), _ = b(e, "width", 8, "1.5rem");
  var l = Br();
  c(l, "stroke-width", 2), C(() => {
    c(l, "width", _()), c(l, "opacity", a());
  }), r(g, l);
}
var zr = m('<div id="tabs" class="svelte-1lf990b"><!> <div role="button" tabindex="0" title="Add New Tab" class="ctrl add-new svelte-1lf990b"><!></div></div> <!>', 1);
function Rr(g, e) {
  _e(e, true);
  let a = q(ne(re[0].id)), _ = oe(() => re.filter((f) => f.id === t(a))[0]);
  de(() => {
    re.length > 0 ? y(a, re[re.length - 1].id, true) : y(a, "");
  });
  function l() {
    re.push({ id: Xe(6), query: $t });
  }
  function s(f) {
    let L = re.map((T) => T.id).indexOf(f);
    t(a) === f ? re.length === 1 ? (re.push(ea), re.shift(), y(a, re[0].id, true)) : L === 0 ? (re.shift(), y(a, re[0].id, true)) : (re.splice(L, 1), y(a, re[L - 1].id, true)) : re.splice(L, 1);
  }
  var x = zr(), k = G(x), p = d(k);
  pe(p, 17, () => re, (f) => f.id, (f, D, L) => {
    Dr(f, { onClose: s, get tab() {
      return t(D).id;
    }, set tab(T) {
      t(D).id = T;
    }, get tabSelected() {
      return t(a);
    }, set tabSelected(T) {
      y(a, T, true);
    }, children: (T, R) => {
      nt();
      var W = at();
      C(() => ve(W, t(D).id)), r(T, W);
    }, $$slots: { default: true } });
  });
  var u = j(p, 2);
  u.__click = l, u.__keydown = l;
  var E = d(u);
  Or(E, {}), i(u), i(k);
  var n = j(k, 2);
  Pr(n, { get query() {
    return t(_);
  } }), r(g, x), be();
}
Ae(["click", "keydown"]);
var Mr = m('<meta property="description" content="Hiqlite Dashboard"/>');
function Wr(g) {
  ta("1uha8ag", (e) => {
    var a = Mr();
    Ft(() => {
      Qt.title = "Hiqlite";
    }), r(e, a);
  }), Rr(g, {});
}
export {
  Wr as component
};
