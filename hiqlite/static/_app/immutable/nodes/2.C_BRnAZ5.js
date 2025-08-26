import "../chunks/NZTpNUN0.js";
import "../chunks/BXesWDf4.js";
import { J as qt, b7 as Ht, aT as Wt, p as _e, i as k, m as c, aZ as De, n as s, t as L, g as t, ab as le, k as a, l as be, a as ve, a8 as A, j as ee, aa as fe, s as z, R as y, b0 as ht, a$ as ye, aG as Ee, Y as oe, o as se, b as Ge, L as at, b3 as ot, ac as nt, b1 as Qt, b2 as Nt } from "../chunks/CYo-iuqb.js";
import { s as h, g as Ce, l as Ke, B as Pe, r as St, a as Ae, b as Zt, f as Se, i as Te, n as Ft, o as Vt, p as Yt, I as Gt, R as Kt, m as yt, q as Xt, j as Jt, Q as ae, u as $t, D as er } from "../chunks/Bc67SYVN.js";
import { p as _, s as tr, a as rr, i as U, b as Ne } from "../chunks/mITizLRE.js";
import { s as ar } from "../chunks/DKol6QFo.js";
function or(n, e, r, d = r) {
  e.addEventListener("input", () => {
    d(e[n]);
  }), qt(() => {
    var l = r();
    if (e[n] !== l) if (l == null) {
      var o = e[n];
      d(o);
    } else e[n] = l + "";
  });
}
function nr(n, e) {
  Ht(window, ["resize"], () => Wt(() => e(window[n])));
}
const lr = () => {
  const n = ar;
  return { page: { subscribe: n.page.subscribe }, navigating: { subscribe: n.navigating.subscribe }, updated: n.updated };
}, ir = { subscribe(n) {
  return lr().page.subscribe(n);
} };
var sr = k('<span class="font-label"><a><!></a></span>');
function xt(n, e) {
  _e(e, true);
  const [r, d] = tr(), l = () => rr(ir, "$page", r);
  let o = _(e, "selectedStep", 3, false), x = _(e, "hideUnderline", 3, false), m = le(() => {
    if (o()) return "step";
    if (l().route.id === e.href.split("?")[0]) return "page";
  });
  var S = sr(), u = c(S);
  let M;
  var i = c(u);
  De(i, () => e.children), s(u), s(S), L((b) => {
    h(u, "href", e.href), h(u, "target", e.target), h(u, "aria-current", t(m)), M = Ce(u, 1, "svelte-a0xtvp", null, M, b);
  }, [() => ({ hideUnderline: x() })]), a(n, S), be(), d();
}
var vr = k('<!> <div class="popover svelte-1au8ouo" popover="auto"><div class="inner fade-in svelte-1au8ouo"><!></div></div>', 1);
function lt(n, e) {
  _e(e, true);
  let r = _(e, "ref", 15), d = _(e, "roleButton", 3, "button"), l = _(e, "offsetLeft", 3, "0px"), o = _(e, "offsetTop", 3, "0px"), x = _(e, "close", 15);
  const m = Ke(8), S = Ke(8);
  let u = A(void 0), M = A(false);
  ve(() => {
    x(b);
  });
  function i() {
    if (r() && t(u)) if (e.absolute) t(u).style.top = o(), t(u).style.left = l();
    else {
      let H = r().getBoundingClientRect();
      t(u).style.top = `calc(${H.bottom + window.scrollY}px + ${o()})`, t(u).style.left = `calc(${H.left + window.scrollX}px + ${l()})`;
    }
    else console.error("button and popover ref missing");
  }
  function b() {
    var _a2;
    (_a2 = t(u)) == null ? void 0 : _a2.hidePopover();
  }
  function O(H) {
    var _a2;
    let V = H.newState;
    y(M, V === "open"), (_a2 = e.onToggle) == null ? void 0 : _a2.call(e, V);
  }
  var C = vr(), N = ee(C);
  Pe(N, { get role() {
    return d();
  }, get id() {
    return m;
  }, get ariaControls() {
    return S;
  }, get popovertarget() {
    return S;
  }, onclick: i, get invisible() {
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
    return r();
  }, set ref(H) {
    r(H);
  }, children: (H, V) => {
    var te = fe(), p = ee(te);
    De(p, () => e.button), a(H, te);
  }, $$slots: { default: true } });
  var R = z(N, 2), D = c(R), K = c(D);
  {
    var E = (H) => {
      var V = fe(), te = ee(V);
      {
        var p = (g) => {
          var P = fe(), w = ee(P);
          De(w, () => e.children), a(g, P);
        };
        U(te, (g) => {
          t(M) && g(p);
        });
      }
      a(H, V);
    }, Z = (H) => {
      var V = fe(), te = ee(V);
      De(te, () => e.children), a(H, V);
    };
    U(K, (H) => {
      e.lazy ? H(E) : H(Z, false);
    });
  }
  s(D), s(R), Ne(R, (H) => y(u, H), () => t(u)), L(() => {
    h(R, "id", S), h(R, "aria-label", e.ariaLabel), h(R, "aria-labelledby", m);
  }), ht("toggle", R, O), a(n, C), be();
}
var cr = ye('<svg fill="none" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M12 9.75L14.25 12m0 0l2.25 2.25M14.25 12l2.25-2.25M14.25 12L12 14.25m-2.58 4.92l-6.375-6.375a1.125 1.125 0 010-1.59L9.42 4.83c.211-.211.498-.33.796-.33H19.5a2.25 2.25 0 012.25 2.25v10.5a2.25 2.25 0 01-2.25 2.25h-9.284c-.298 0-.585-.119-.796-.33z"></path></svg>');
function dr(n, e) {
  let r = _(e, "color", 8, "currentColor"), d = _(e, "opacity", 8, 0.9), l = _(e, "width", 8, "1.5rem");
  var o = cr();
  h(o, "stroke-width", 2), L(() => {
    h(o, "stroke", r()), h(o, "width", l()), h(o, "opacity", d());
  }), a(n, o);
}
var ur = ye('<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M21 21l-5.197-5.197m0 0A7.5 7.5 0 105.196 5.196a7.5 7.5 0 0010.607 10.607z"></path></svg>');
function fr(n, e) {
  let r = _(e, "opacity", 8, 0.9), d = _(e, "width", 8, "1.5rem");
  var l = ur();
  h(l, "stroke-width", 2), L(() => {
    h(l, "width", d()), h(l, "opacity", r());
  }), a(n, l);
}
function hr(n, e, r, d) {
  var _a2, _b, _c;
  switch (n.code) {
    case "Enter":
      e();
      break;
    case "Tab":
      (_a2 = r.onTab) == null ? void 0 : _a2.call(r, d());
      break;
    case "ArrowUp":
      (_b = r.onUp) == null ? void 0 : _b.call(r, d());
      break;
    case "ArrowDown":
      (_c = r.onDown) == null ? void 0 : _c.call(r, d());
      break;
  }
}
var gr = k('<div class="options svelte-13lxusw"><!></div>'), _r = k("<option></option>"), br = k('<datalist class="absolute svelte-13lxusw"></datalist>'), wr = k('<div class="magnify svelte-13lxusw"><!></div>'), mr = k('<div class="btnSearch svelte-13lxusw"><!></div>'), kr = k('<search class="flex container svelte-13lxusw"><!> <input type="search" autocomplete="off" aria-label="Search" placeholder="Search" class="svelte-13lxusw"/> <!> <div class="relative"><div class="absolute btnDelete svelte-13lxusw"><!></div></div> <!></search>');
function yr(n, e) {
  _e(e, true);
  let r = _(e, "value", 15, ""), d = _(e, "option", 15), l = _(e, "focus", 15), o = _(e, "width", 3, "100%");
  const x = Ke(8), m = Ke(8);
  let S = A(void 0), u = le(() => e.datalist && e.datalist.length > 0 ? m : void 0);
  ve(() => {
    l(i);
  });
  function M() {
    var _a2;
    (_a2 = e.onSearch) == null ? void 0 : _a2.call(e, r());
  }
  function i() {
    var _a2;
    (_a2 = t(S)) == null ? void 0 : _a2.focus();
  }
  var b = kr();
  let O;
  var C = c(b);
  {
    var N = (p) => {
      var g = gr(), P = c(g);
      Pt(P, { ariaLabel: "Search Options", get options() {
        return e.options;
      }, borderless: true, get value() {
        return d();
      }, set value(w) {
        d(w);
      } }), s(g), a(p, g);
    };
    U(C, (p) => {
      e.options && p(N);
    });
  }
  var R = z(C, 2);
  St(R), R.__keydown = [hr, M, e, r], Ne(R, (p) => y(S, p), () => t(S));
  var D = z(R, 2);
  {
    var K = (p) => {
      var g = br();
      Se(g, 21, () => e.datalist, Te, (P, w, W, j) => {
        var F = _r(), he = {};
        L(() => {
          he !== (he = t(w)) && (F.value = (F.__value = t(w)) ?? "");
        }), a(P, F);
      }), s(g), L(() => h(g, "id", m)), a(p, g);
    };
    U(D, (p) => {
      e.datalist && p(K);
    });
  }
  var E = z(D, 2), Z = c(E), H = c(Z);
  Pe(H, { ariaLabel: "Delete Search Input", invisible: true, onclick: () => r(""), children: (p, g) => {
    dr(p, { color: "hsl(var(--bg-high))", width: 24 });
  }, $$slots: { default: true } }), s(Z), s(E);
  var V = z(E, 2);
  {
    var te = (p) => {
      var g = mr(), P = c(g);
      Pe(P, { ariaLabel: "Search", invisible: true, onclick: M, children: (w, W) => {
        var j = wr(), F = c(j);
        fr(F, {}), s(j), a(w, j);
      }, $$slots: { default: true } }), s(g), a(p, g);
    };
    U(V, (p) => {
      e.onSearch && p(te);
    });
  }
  s(b), L((p) => {
    O = Ae(b, "", O, p), h(R, "id", x), h(R, "list", t(u));
  }, [() => ({ border: e.borderless ? void 0 : "1px solid hsl(var(--bg-high))", width: o() })]), ht("focus", R, () => {
    var _a2;
    return (_a2 = e.onFocus) == null ? void 0 : _a2.call(e);
  }), Zt(R, r), a(n, b), be();
}
Ee(["keydown"]);
var xr = ye('<svg fill="none" viewBox="0 0 24 24" color="currentColor" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M8.25 4.5l7.5 7.5-7.5 7.5"></path></svg>');
function ft(n, e) {
  let r = _(e, "opacity", 8, 0.9), d = _(e, "width", 8, "1.5rem");
  var l = xr();
  h(l, "stroke-width", 2), L(() => {
    h(l, "width", d()), h(l, "opacity", r());
  }), a(n, l);
}
function pr(n, e, r, d, l) {
  let o = n.code;
  o === "ArrowDown" ? (n.preventDefault(), e() && y(r, t(r) + 1)) : o === "ArrowUp" ? (n.preventDefault(), e() && y(r, t(r) - 1)) : o === "Enter" && t(r) > -1 ? d(t(l)[t(r)]) : o === "Enter" && t(r) === -1 && t(l).length === 1 && d(t(l)[0]);
}
var Cr = k('<div class="btn svelte-1j5gmms"> <div class="chevron svelte-1j5gmms"><!></div></div>'), Sr = k('<div class="optPopover svelte-1j5gmms"> </div>'), Pr = k('<div role="listbox" tabindex="0" class="popover svelte-1j5gmms"><!> <div class="popoverOptions svelte-1j5gmms"></div></div>'), Lr = k('<option class="opt svelte-1j5gmms"> </option>'), Tr = k("<select></select>");
function Pt(n, e) {
  _e(e, true);
  let r = _(e, "ref", 15), d = _(e, "options", 19, () => []), l = _(e, "value", 15), o = _(e, "asPopover", 3, true), x = _(e, "borderless", 3, false), m = _(e, "withSearch", 3, false), S = _(e, "fallbackOptions", 3, false), u = A(void 0), M = A(oe(S() ? false : o())), i = A(void 0), b = A(oe(m() ? -1 : 0)), O = A(void 0), C = A(""), N = le(() => {
    if (!m()) return d();
    if (typeof l() == "string") return d().filter((g) => g.toLowerCase().includes(t(C).toLowerCase()));
    let p = Number.parseInt(t(C)) || l();
    return d().filter((g) => g === p);
  });
  ve(() => {
    t(M) !== o() && y(M, o());
  }), ve(() => {
    var _a2, _b;
    if (t(b) === -1 && ((_a2 = t(u)) == null ? void 0 : _a2.scrollTo({ top: 0, behavior: "smooth" })), m()) {
      if (t(b) < 0 || t(b) > t(N).length - 1) {
        y(b, -1), (_b = t(O)) == null ? void 0 : _b();
        return;
      }
    } else t(b) < 0 ? y(b, t(N).length - 1) : t(b) > t(N).length - 1 && y(b, 0), R();
  });
  function R() {
    if (t(u)) {
      let p = t(u).getElementsByTagName("button")[t(b)];
      p.scrollIntoView({ behavior: "smooth", block: "center" }), p.focus();
    } else console.error("refOptions is undefined");
  }
  function D(p) {
    var _a2;
    p === "open" && (m() ? (y(b, -1), (_a2 = t(O)) == null ? void 0 : _a2()) : (y(b, d().findIndex((g) => g === l()) || 0, true), R()));
  }
  function K() {
    return t(N).length > 0 ? true : (y(b, -1), false);
  }
  function E(p) {
    l(p), y(C, ""), setTimeout(() => {
      var _a2;
      (_a2 = t(i)) == null ? void 0 : _a2();
    }, 20);
  }
  var Z = fe(), H = ee(Z);
  {
    var V = (p) => {
      lt(p, { get ariaLabel() {
        return e.ariaLabel;
      }, roleButton: "combobox", btnInvisible: true, get offsetTop() {
        return e.offsetTop;
      }, get offsetLeft() {
        return e.offsetLeft;
      }, onToggle: D, get onLeft() {
        return e.onLeft;
      }, get onRight() {
        return e.onRight;
      }, get onUp() {
        return e.onUp;
      }, get onDown() {
        return e.onDown;
      }, get ref() {
        return r();
      }, set ref(P) {
        r(P);
      }, get close() {
        return t(i);
      }, set close(P) {
        y(i, P, true);
      }, button: (P) => {
        var w = Cr(), W = c(w), j = z(W), F = c(j);
        ft(F, { width: 14 }), s(j), s(w), L(() => {
          h(w, "data-border", !x()), se(W, `${l() ?? ""} `);
        }), a(P, w);
      }, children: (P, w) => {
        var W = Pr();
        W.__keydown = [pr, K, b, E, N];
        let j;
        var F = c(W);
        {
          var he = (ce) => {
            yr(ce, { onFocus: () => y(b, -1), get value() {
              return t(C);
            }, set value(X) {
              y(C, X, true);
            }, get focus() {
              return t(O);
            }, set focus(X) {
              y(O, X, true);
            } });
          };
          U(F, (ce) => {
            m() && ce(he);
          });
        }
        var xe = z(F, 2);
        Se(xe, 21, () => t(N), Te, (ce, X, je) => {
          Pe(ce, { invisible: true, invisibleOutline: true, onclick: () => E(t(X)), children: (Ze, Xe) => {
            var Ie = Sr(), Je = c(Ie, true);
            s(Ie), L(() => {
              h(Ie, "aria-selected", l() === t(X)), h(Ie, "data-focus", t(b) === je), se(Je, t(X));
            }), a(Ze, Ie);
          }, $$slots: { default: true } });
        }), s(xe), Ne(xe, (ce) => y(u, ce), () => t(u)), s(W), L((ce) => j = Ae(W, "", j, ce), [() => ({ "max-height": e.maxHeight })]), a(P, W);
      }, $$slots: { button: true, default: true } });
    }, te = (p) => {
      var g = Tr();
      let P;
      Se(g, 21, () => t(N), Te, (w, W) => {
        var j = Lr(), F = c(j, true);
        s(j);
        var he = {};
        L(() => {
          Ft(j, l() === t(W)), se(F, t(W)), he !== (he = t(W)) && (j.value = (j.__value = t(W)) ?? "");
        }), a(w, j);
      }), s(g), L((w) => {
        h(g, "name", e.name), h(g, "aria-label", e.ariaLabel), P = Ce(g, 1, "svelte-1j5gmms", null, P, w);
      }, [() => ({ borderless: x() })]), Vt(g, l), a(p, g);
    };
    U(H, (p) => {
      t(M) ? p(V) : p(te, false);
    });
  }
  a(n, Z), be();
}
Ee(["keydown"]);
var Dr = k('<div class="link noselect svelte-1bye1t3"> </div>'), Ir = k('<li class="svelte-1bye1t3"><!></li>'), Br = k('<nav aria-label="Pagination" class="svelte-1bye1t3"><ul class="svelte-1bye1t3"></ul></nav>'), Or = k('<div class="flex gap-10 svelte-1bye1t3"><div class="flex gap-05 chunkSize noselect svelte-1bye1t3"><div class="svelte-1bye1t3">Entries</div> <div class="svelte-1bye1t3"><!></div></div> <div class="font-label total svelte-1bye1t3"> </div></div>'), Rr = k('<div class="iconLeft svelte-1bye1t3" aria-label="Go to previous page"><!></div>'), zr = k('<div class="iconRight svelte-1bye1t3" aria-label="Go to next page"><!></div>'), Mr = k('<div class="container svelte-1bye1t3"><!> <!> <!> <!></div>');
function Ar(n, e) {
  _e(e, true);
  const r = (g) => {
    var P = Br(), w = c(P);
    Se(w, 21, () => t(b), Te, (W, j) => {
      var F = Ir(), he = c(F);
      Pe(he, { invisible: true, onclick: () => N(t(j)), onLeft: O, onRight: C, children: (xe, ce) => {
        var X = Dr(), je = c(X, true);
        s(X), L(() => se(je, t(j))), a(xe, X);
      }, $$slots: { default: true } }), s(F), L(() => {
        h(F, "aria-label", `go to page number: ${t(j)}`), h(F, "aria-current", x() === t(j) ? "step" : void 0);
      }), a(W, F);
    }), s(w), s(P), a(g, P);
  }, d = (g) => {
    var P = Or(), w = c(P), W = z(c(w), 2), j = c(W);
    Pt(j, { ariaLabel: "Page Count", get options() {
      return l;
    }, offsetTop: "-17rem", borderless: true, get value() {
      return m();
    }, set value(xe) {
      m(xe);
    } }), s(W), s(w);
    var F = z(w, 2), he = c(F);
    s(F), s(P), L(() => se(he, `Total: ${e.items.length ?? ""}`)), a(g, P);
  }, l = [5, 7, 10, 15, 20, 30, 50, 100];
  let o = _(e, "itemsPaginated", 15), x = _(e, "page", 15, 1), m = _(e, "pageSize", 31, () => oe(l[0])), S = _(e, "compact", 3, false);
  const u = 16;
  let M = Ge(() => m()), i = A(oe([])), b = A(oe([]));
  ve(() => {
    m() !== M && (M = Ge(() => m()), x(1));
  }), ve(() => {
    let g = [];
    for (let P = 0; P < e.items.length; P += m()) {
      const w = e.items.slice(P, P + m());
      g.push(w);
    }
    y(i, g, true), o(g[x() - 1]);
  }), ve(() => {
    R();
  });
  function O() {
    x() > 1 && N(x() - 1);
  }
  function C() {
    x() < t(i).length && N(x() + 1);
  }
  function N(g) {
    x(g), R();
  }
  function R() {
    let g = [], P = Math.floor(m() / 2);
    if (t(i).length <= m()) for (let w = 1; w <= t(i).length; w++) g.push(w);
    else if (x() <= P) for (let w = 1; w <= m(); w++) g.push(w);
    else if (x() > t(i).length - P - 1) for (let w = t(i).length - m(); w <= t(i).length - 1; w++) g.push(w + 1);
    else for (let w = x() - P; w < x() - P + m(); w++) g.push(w);
    y(b, g, true);
  }
  var D = Mr(), K = c(D);
  const E = le(() => x() === 1);
  Pe(K, { onclick: O, invisible: true, get isDisabled() {
    return t(E);
  }, children: (g, P) => {
    var w = Rr(), W = c(w);
    ft(W, { width: u }), s(w), L(() => h(w, "data-disabled", x() === 1)), a(g, w);
  }, $$slots: { default: true } });
  var Z = z(K, 2);
  r(Z);
  var H = z(Z, 2);
  const V = le(() => x() === t(i).length);
  Pe(H, { onclick: C, invisible: true, get isDisabled() {
    return t(V);
  }, children: (g, P) => {
    var w = zr(), W = c(w);
    ft(W, { width: u }), s(w), L(() => h(w, "data-disabled", x() === t(i).length)), a(g, w);
  }, $$slots: { default: true } });
  var te = z(H, 2);
  {
    var p = (g) => {
      d(g);
    };
    U(te, (g) => {
      S() || g(p);
    });
  }
  s(D), a(n, D), be();
}
function Er(n, e) {
  console.log(n.code), n.code === "Enter" && e();
}
var jr = k('<label class="font-label noselect svelte-1supmpl"><input type="checkbox" class="svelte-1supmpl"/> <span class="svelte-1supmpl"><!></span></label>');
function Ye(n, e) {
  _e(e, true);
  let r = _(e, "checked", 15, false), d = _(e, "ariaLabel", 3, ""), l = _(e, "borderColor", 3, "hsl(var(--bg-high))");
  function o() {
    r(!r());
  }
  var x = jr(), m = c(x);
  St(m), m.__click = o, m.__keydown = [Er, o];
  let S;
  var u = z(m, 2), M = c(u);
  De(M, () => e.children ?? at), s(u), s(x), L((i) => {
    h(m, "name", e.name), m.disabled = e.disabled, h(m, "aria-disabled", e.disabled), h(m, "aria-checked", r()), h(m, "aria-label", d()), S = Ae(m, "", S, i);
  }, [() => ({ "border-color": l() })]), Yt(m, r), a(n, x), be();
}
Ee(["click", "keydown"]);
var Ur = ye('<svg fill="none" viewBox="0 0 24 24" stroke="currentColor" color="hsl(var(--action))"><path stroke-linecap="round" stroke-linejoin="round" d="M4.5 12.75l6 6 9-13.5"></path></svg>'), qr = ye('<svg fill="none" viewBox="0 0 24 24" stroke="currentColor" color="hsl(var(--error))"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12"></path></svg>');
function Hr(n, e) {
  _e(e, true);
  const r = 20, d = 0.9;
  var l = fe(), o = ee(l);
  {
    var x = (S) => {
      var u = Ur();
      h(u, "stroke-width", 2), h(u, "width", r), h(u, "opacity", d), a(S, u);
    }, m = (S) => {
      var u = qr();
      h(u, "stroke-width", 2), h(u, "width", r), h(u, "opacity", d), a(S, u);
    };
    U(o, (S) => {
      e.checked ? S(x) : S(m, false);
    });
  }
  a(n, l), be();
}
var Wr = ye('<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m19.5 8.25-7.5 7.5-7.5-7.5"></path></svg>');
function Qr(n, e) {
  let r = _(e, "color", 8, "currentColor"), d = _(e, "opacity", 8, 0.9), l = _(e, "width", 8, "1.5rem");
  var o = Wr();
  h(o, "stroke-width", 2), L(() => {
    h(o, "width", l()), h(o, "color", r()), h(o, "opacity", d());
  }), a(n, o);
}
var Nr = ye('<svg fill="none" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M3 7.5 7.5 3m0 0L12 7.5M7.5 3v13.5m13.5 0L16.5 21m0 0L12 16.5m4.5 4.5V7.5"></path></svg>');
function Zr(n, e) {
  let r = _(e, "color", 8, "currentColor"), d = _(e, "opacity", 8, 0.9), l = _(e, "width", 8, "1.5rem");
  var o = Nr();
  h(o, "stroke-width", 2), L(() => {
    h(o, "stroke", r()), h(o, "width", l()), h(o, "opacity", d());
  }), a(n, o);
}
var Fr = ye(`<svg fill="none" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213
            1.281c.063.374.313.686.645.87.074.04.147.083.22.127.325.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 0
            1 1.37.49l1.296 2.247a1.125 1.125 0 0 1-.26 1.431l-1.003.827c-.293.241-.438.613-.43.992a7.723 7.723 0 0
            1 0 .255c-.008.378.137.75.43.991l1.004.827c.424.35.534.955.26 1.43l-1.298 2.247a1.125 1.125 0 0
            1-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.47 6.47 0 0
            1-.22.128c-.331.183-.581.495-.644.869l-.213 1.281c-.09.543-.56.94-1.11.94h-2.594c-.55 0-1.019-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52
            0 0 1-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 0 1-1.369-.49l-1.297-2.247a1.125
            1.125 0 0 1 .26-1.431l1.004-.827c.292-.24.437-.613.43-.991a6.932 6.932 0 0 1 0-.255c.007-.38-.138-.751-.43-.992l-1.004-.827a1.125 1.125
            0 0 1-.26-1.43l1.297-2.247a1.125 1.125 0 0 1 1.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.086.22-.128.332-.183.582-.495.644-.869l.214-1.28Z"></path><path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z"></path></svg>`);
function Vr(n, e) {
  let r = _(e, "color", 8, "currentColor"), d = _(e, "opacity", 8, 0.9), l = _(e, "width", 8, "1.5rem");
  var o = Fr();
  h(o, "stroke-width", 2), L(() => {
    h(o, "stroke", r()), h(o, "width", l()), h(o, "opacity", d());
  }), a(n, o);
}
async function Yr(n) {
  var _a2;
  await ((_a2 = navigator == null ? void 0 : navigator.clipboard) == null ? void 0 : _a2.writeText(n));
}
var Gr = ye(`<svg fill="none" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M6.75 12a.75.75 0 1 1-1.5 0 .75.75 0 0 1 1.5 0ZM12.75 12a.75.75 0 1 1-1.5 0 .75.75 0 0 1 1.5
            0ZM18.75 12a.75.75 0 1 1-1.5 0 .75.75 0 0 1 1.5 0Z"></path></svg>`);
function Kr(n, e) {
  let r = _(e, "color", 8, "currentColor"), d = _(e, "opacity", 8, 0.9), l = _(e, "width", 8, "1.5rem");
  var o = Gr();
  h(o, "stroke-width", 2), L(() => {
    h(o, "stroke", r()), h(o, "width", l()), h(o, "opacity", d());
  }), a(n, o);
}
var Xr = k('<span class="btnSelect svelte-12u4ifk"><!></span>'), Jr = k('<th class="headerCheckbox svelte-12u4ifk"><!> <!></th>'), $r = k('<span class="iconOrder svelte-12u4ifk"><!></span>'), ea = k('<span class="orderText svelte-12u4ifk"> </span> <!>', 1), ta = k('<span class="rawText svelte-12u4ifk"> </span>'), ra = k('<th class="svelte-12u4ifk"><span class="flex-1 label svelte-12u4ifk"><!></span> <span class="relative"><span role="none" class="absolute sizeable svelte-12u4ifk"></span></span></th>'), aa = k('<th class="headerOptions svelte-12u4ifk"><!></th>'), oa = k('<td class="checkbox svelte-12u4ifk"><!></td>'), na = k("<span> </span>"), la = k("<span> </span>"), ia = k("<span> </span>"), sa = k("<span><!></span>"), va = k("<span> </span>"), ca = k("<span> </span>"), da = k('<td class="svelte-12u4ifk"><!></td>'), ua = k('<span class="btnOptions svelte-12u4ifk"><!></span>'), fa = k('<td class="options svelte-12u4ifk"><!></td>'), ha = k("<tr><!><!><!></tr>"), ga = k('<div class="eye svelte-12u4ifk"><!></div>'), _a = k('<div class="columnSelect svelte-12u4ifk"><!></div>'), ba = k('<div class="columnSelect svelte-12u4ifk"><!></div>'), wa = k('<div class="columnSelect svelte-12u4ifk"><!></div>'), ma = k('<div class="columnsSelect svelte-12u4ifk"><!> <!> <!></div>'), ka = k("<div></div>"), ya = k('<table class="svelte-12u4ifk"><thead class="svelte-12u4ifk"><tr class="svelte-12u4ifk"><!><!><!></tr></thead><tbody class="svelte-12u4ifk"><!></tbody><caption class="flex space-between svelte-12u4ifk"><div class="flex"><!> <div class="caption svelte-12u4ifk"><!></div> <!></div></caption></table>');
function xa(n, e) {
  _e(e, true);
  let r = _(e, "showColumns", 31, () => oe(Array(e.columns.length).fill(true))), d = _(e, "paginationCompact", 3, false), l = _(e, "paginationPageSize", 3, 15), o = _(e, "selectInitHide", 3, false), x = _(e, "minWidthColPx", 3, 50);
  const m = "3rem", S = "2rem";
  let u = oe(P()), M = A(oe(Ge(() => u))), i = A(1), b = A(oe(Ge(() => l()))), O = A(false), C = A(oe(Array(e.rows.length).fill(false))), N = le(() => t(C).find((v) => v === true)), R = A(void 0), D = A(oe([])), K = A("up"), E = A(oe([])), Z = le(() => e.paginationDisabled !== void 0 ? e.paginationDisabled : e.rows.length < l()), H = le(() => e.paginationDisabled || t(Z) ? e.rows.length : t(E) && t(E).length ? t(E).length : (t(i) != 1 && y(i, 1), 0)), V = oe(Array(Ge(() => u.length)).fill(void 0)), te = A(void 0), p = 0, g = A(void 0);
  ve(() => {
    setTimeout(() => {
      for (let v = 1; v < u.length; v++) if (u[v] === "auto") {
        p = e.select ? v : v - 1;
        let f = V[v];
        f && ce(f.getBoundingClientRect().width);
      }
    }, 150);
  }), ve(() => {
    let v = Array(e.rows.length).fill(false);
    if (t(O)) {
      let f;
      t(i) === 1 ? f = 0 : f = (t(i) - 1) * t(b);
      let T = Math.min(t(i) * t(b), e.rows.length);
      for (let B = f; B < T; B++) v[B] = true;
    }
    y(C, v, true);
  }), ve(() => {
    let v = e.paginationDisabled || t(Z) ? e.rows.length : t(E).length || 0;
    y(D, Array(v).fill(() => console.error("un-initialized popover close option")), true);
  }), ve(() => {
    let v = [];
    for (let f = 0; f < u.length; f++) r()[f] && v.push(u[f]);
    y(M, v, true);
  }), ve(() => {
    if (e.highlight !== void 0 && t(te)) {
      let v = e.highlight;
      !t(Z) && t(i) > 1 && (v = e.highlight - (t(i) - 1) * t(b)), y(g, v, true), setTimeout(() => {
        var _a2, _b;
        (_b = (_a2 = t(te)) == null ? void 0 : _a2.getElementsByClassName("highlight")[0]) == null ? void 0 : _b.scrollIntoView({ behavior: "smooth", block: "center" });
      }, 250);
    } else y(g, void 0);
  });
  function P() {
    let v = e.columns.map((T) => T.initialWidth), f = [...r()];
    return e.select && (v = [m, ...v], f = [!o(), ...f]), e.options && (v = [...v, S], f = [...f, true]), r(f), v;
  }
  function w() {
    return t(M).join(" ");
  }
  function W(v, f) {
    y(C, Array(e.rows.length).fill(false), true);
    let T = 1;
    t(K) === "up" ? (T = -1, y(K, "down")) : y(K, "up"), f === "string" ? e.rows.sort((B, Y) => B[v].content.localeCompare(Y[v].content) * T) : f === "number" && e.rows.sort((B, Y) => (B[v].content - Y[v].content) * T);
  }
  function j(v) {
    return !t(Z) && t(i) > 1 ? (t(i) - 1) * t(b) + v : v;
  }
  function F(v) {
    p = v;
    let f = V[v];
    f ? (ce(f.getBoundingClientRect().width), window.addEventListener("mousemove", xe), window.addEventListener("mouseup", he, { once: true })) : console.error("invalid ref from refCols in onMouseDown");
  }
  function he() {
    window.removeEventListener("mousemove", xe);
  }
  function xe(v) {
    let f = V[p];
    if (f) {
      let T = f.getBoundingClientRect().left, B = window.scrollX + v.x - T;
      ce(B);
    } else console.error("invalid ref from refCols in onMove");
  }
  function ce(v) {
    v = Math.ceil(v), v < x() && (v = x()), u[e.select ? p + 1 : p] = `${v}px`;
  }
  var X = ya();
  let je;
  var Ze = c(X), Xe = c(Ze);
  let Ie;
  var Je = c(Xe);
  {
    var Lt = (v) => {
      var f = Jr(), T = c(f);
      Ye(T, { ariaLabel: "Select All", borderColor: "hsla(var(--text), .4)", get checked() {
        return t(O);
      }, set checked(ie) {
        y(O, ie, true);
      } });
      var B = z(T, 2);
      const Y = le(() => !t(N));
      lt(B, { ariaLabel: "Selected Options", get btnDisabled() {
        return t(Y);
      }, btnInvisible: true, get close() {
        return t(R);
      }, set close(G) {
        y(R, G, true);
      }, button: (G) => {
        var re = Xr(), de = c(re);
        Qr(de, { width: "1rem" }), s(re), L(() => h(re, "data-disabled", !t(N))), a(G, re);
      }, children: (G, re) => {
        var de = fe(), J = ee(de);
        De(J, () => e.select, () => t(C), () => t(R)), a(G, de);
      }, $$slots: { button: true, default: true } }), s(f), a(v, f);
    };
    U(Je, (v) => {
      e.select && r()[0] && v(Lt);
    });
  }
  var gt = z(Je);
  Se(gt, 17, () => e.columns, Te, (v, f, T) => {
    var B = fe(), Y = ee(B);
    {
      var ie = (G) => {
        var re = ra(), de = c(re), J = c(de);
        {
          var ne = (q) => {
            var $ = ea(), ue = ee($), we = c(ue, true);
            s(ue);
            var pe = z(ue, 2);
            Pe(pe, { invisible: true, onclick: () => W(T, t(f).orderType), children: (Me, mt) => {
              var Fe = $r(), st = c(Fe);
              Zr(st, { width: "1rem" }), s(Fe), a(Me, Fe);
            }, $$slots: { default: true } }), L(() => se(we, t(f).content)), a(q, $);
          }, ge = (q) => {
            var $ = ta(), ue = c($, true);
            s($), L(() => se(ue, t(f).content)), a(q, $);
          };
          U(J, (q) => {
            t(f).orderType ? q(ne) : q(ge, false);
          });
        }
        s(de);
        var Q = z(de, 2), I = c(Q);
        I.__mousedown = () => F(T), s(Q), s(re), Ne(re, (q, $) => V[$] = q, (q) => V == null ? void 0 : V[q], () => [T]), a(G, re);
      };
      U(Y, (G) => {
        r()[e.select ? T + 1 : T] && G(ie);
      });
    }
    a(v, B);
  });
  var Tt = z(gt);
  {
    var Dt = (v) => {
      var f = aa(), T = c(f);
      Vr(T, { width: "1.2rem" }), s(f), a(v, f);
    };
    U(Tt, (v) => {
      e.options && r()[r().length - 1] && v(Dt);
    });
  }
  s(Xe), s(Ze);
  var $e = z(Ze);
  {
    const v = (f, T = at, B = at) => {
      var Y = ha();
      let ie, G;
      var re = c(Y);
      {
        var de = (Q) => {
          var I = oa(), q = c(I);
          Ye(q, { ariaLabel: "Select Row", get checked() {
            return t(C)[j(B())];
          }, set checked($) {
            t(C)[j(B())] = $;
          } }), s(I), a(Q, I);
        };
        U(re, (Q) => {
          e.select && r()[0] && Q(de);
        });
      }
      var J = z(re);
      Se(J, 17, T, Te, (Q, I, q) => {
        var $ = fe(), ue = ee($);
        {
          var we = (pe) => {
            var Me = da(), mt = c(Me);
            {
              var Fe = (Ue) => {
                const vt = le(() => t(I).href || "");
                xt(Ue, { get href() {
                  return t(vt);
                }, children: (ct, kt) => {
                  var me = na();
                  let qe;
                  var Ve = c(me, true);
                  s(me), L((et) => {
                    qe = Ce(me, 1, "linkText nowrap svelte-12u4ifk", null, qe, et), se(Ve, t(I).content);
                  }, [() => ({ muted: t(I).muted })]), a(ct, me);
                }, $$slots: { default: true } });
              }, st = (Ue, vt) => {
                {
                  var ct = (me) => {
                    const qe = le(() => t(I).href || "");
                    xt(me, { get href() {
                      return t(qe);
                    }, target: "_blank", children: (Ve, et) => {
                      var ke = la();
                      let He;
                      var tt = c(ke, true);
                      s(ke), L((Be) => {
                        He = Ce(ke, 1, "linkText nowrap svelte-12u4ifk", null, He, Be), se(tt, t(I).content);
                      }, [() => ({ muted: t(I).muted })]), a(Ve, ke);
                    }, $$slots: { default: true } });
                  }, kt = (me, qe) => {
                    {
                      var Ve = (ke) => {
                        Pe(ke, { invisible: true, onclick: () => Yr(t(I).content.toString()), children: (He, tt) => {
                          var Be = ia();
                          let Le;
                          var Oe = c(Be, true);
                          s(Be), L((We) => {
                            Le = Ce(Be, 1, "copyToClip nowrap svelte-12u4ifk", null, Le, We), se(Oe, t(I).content);
                          }, [() => ({ muted: t(I).muted })]), a(He, Be);
                        }, $$slots: { default: true } });
                      }, et = (ke, He) => {
                        {
                          var tt = (Le) => {
                            var Oe = sa();
                            let We;
                            var dt = c(Oe);
                            Hr(dt, { get checked() {
                              return t(I).content;
                            } }), s(Oe), L((Re) => We = Ce(Oe, 1, "checkIcon nowrap svelte-12u4ifk", null, We, Re), [() => ({ muted: t(I).muted })]), a(Le, Oe);
                          }, Be = (Le, Oe) => {
                            {
                              var We = (Re) => {
                                Pe(Re, { invisible: true, onclick: (ze) => {
                                  var _a2, _b;
                                  return (_b = (_a2 = t(I)).onClick) == null ? void 0 : _b.call(_a2, ze, j(B()));
                                }, children: (ze, ut) => {
                                  var Qe = va();
                                  let rt;
                                  var jt = c(Qe, true);
                                  s(Qe), L((Ut) => {
                                    rt = Ce(Qe, 1, "onclick nowrap svelte-12u4ifk", null, rt, Ut), se(jt, t(I).content);
                                  }, [() => ({ muted: t(I).muted })]), a(ze, Qe);
                                }, $$slots: { default: true } });
                              }, dt = (Re) => {
                                var ze = ca();
                                let ut;
                                var Qe = c(ze, true);
                                s(ze), L((rt) => {
                                  ut = Ce(ze, 1, "rawText nowrap svelte-12u4ifk", null, ut, rt), se(Qe, t(I).content);
                                }, [() => ({ muted: t(I).muted })]), a(Re, ze);
                              };
                              U(Le, (Re) => {
                                t(I).onClick ? Re(We) : Re(dt, false);
                              }, Oe);
                            }
                          };
                          U(ke, (Le) => {
                            var _a2;
                            ((_a2 = e.columns[q]) == null ? void 0 : _a2.showAs) === "check" ? Le(tt) : Le(Be, false);
                          }, He);
                        }
                      };
                      U(me, (ke) => {
                        var _a2;
                        ((_a2 = e.columns[q]) == null ? void 0 : _a2.showAs) === "copyToClip" ? ke(Ve) : ke(et, false);
                      }, qe);
                    }
                  };
                  U(Ue, (me) => {
                    var _a2;
                    ((_a2 = e.columns[q]) == null ? void 0 : _a2.showAs) === "a_blank" ? me(ct) : me(kt, false);
                  }, vt);
                }
              };
              U(mt, (Ue) => {
                var _a2;
                ((_a2 = e.columns[q]) == null ? void 0 : _a2.showAs) === "a" ? Ue(Fe) : Ue(st, false);
              });
            }
            s(Me), a(pe, Me);
          };
          U(ue, (pe) => {
            r()[e.select ? q + 1 : q] && pe(we);
          });
        }
        a(Q, $);
      });
      var ne = z(J);
      {
        var ge = (Q) => {
          var I = fa(), q = c(I);
          lt(q, { ariaLabel: "Options", btnInvisible: true, get offsetLeft() {
            return e.offsetLeftOptions;
          }, get offsetTop() {
            return e.offsetTopOptions;
          }, get close() {
            return t(D)[B()];
          }, set close(ue) {
            t(D)[B()] = ue;
          }, button: (ue) => {
            var we = ua(), pe = c(we);
            Kr(pe, {}), s(we), a(ue, we);
          }, children: (ue, we) => {
            var pe = fe(), Me = ee(pe);
            De(Me, () => e.options, T, () => t(D)[B()]), a(ue, pe);
          }, $$slots: { button: true, default: true } }), s(I), a(Q, I);
        };
        U(ne, (Q) => {
          e.options && r()[r().length - 1] && Q(ge);
        });
      }
      s(Y), L((Q, I) => {
        ie = Ce(Y, 1, "svelte-12u4ifk", null, ie, Q), G = Ae(Y, "", G, I);
      }, [() => ({ highlight: t(g) === B() }), () => ({ "grid-template-columns": w() })]), a(f, Y);
    };
    var It = c($e);
    {
      var Bt = (f) => {
        var T = fe(), B = ee(T);
        {
          var Y = (ie) => {
            var G = fe(), re = ee(G);
            Se(re, 17, () => e.rows, Te, (de, J, ne) => {
              v(de, () => t(J), () => ne);
            }), a(ie, G);
          };
          U(B, (ie) => {
            t(C).length === e.rows.length && ie(Y);
          });
        }
        a(f, T);
      }, Ot = (f) => {
        var T = fe(), B = ee(T);
        Se(B, 17, () => t(E), Te, (Y, ie, G) => {
          v(Y, () => t(ie), () => G);
        }), a(f, T);
      };
      U(It, (f) => {
        t(Z) ? f(Bt) : f(Ot, false);
      });
    }
    s($e), Ne($e, (f) => y(te, f), () => t(te));
  }
  var _t = z($e), bt = c(_t), wt = c(bt);
  const Rt = le(() => `-${u.length * 1.4 + 3}rem`);
  lt(wt, { ariaLabel: "Select Columns", get offsetTop() {
    return t(Rt);
  }, btnInvisible: true, button: (f) => {
    var T = ga(), B = c(T);
    Gt(B, {}), s(T), a(f, T);
  }, children: (f, T) => {
    var B = ma(), Y = c(B);
    {
      var ie = (J) => {
        var ne = _a(), ge = c(ne);
        Ye(ge, { ariaLabel: "Select Column: Select", get checked() {
          return r()[0];
        }, set checked(Q) {
          r(r()[0] = Q, true);
        }, children: (Q, I) => {
          ot();
          var q = nt("Select");
          a(Q, q);
        }, $$slots: { default: true } }), s(ne), a(J, ne);
      };
      U(Y, (J) => {
        e.select && J(ie);
      });
    }
    var G = z(Y, 2);
    Se(G, 17, () => e.columns, Te, (J, ne, ge) => {
      var Q = ba(), I = c(Q);
      const q = le(() => `Select Column: ${t(ne).content}`);
      Ye(I, { get ariaLabel() {
        return t(q);
      }, get checked() {
        return r()[e.select ? ge + 1 : ge];
      }, set checked($) {
        r(r()[e.select ? ge + 1 : ge] = $, true);
      }, children: ($, ue) => {
        ot();
        var we = nt();
        L(() => se(we, t(ne).content)), a($, we);
      }, $$slots: { default: true } }), s(Q), a(J, Q);
    });
    var re = z(G, 2);
    {
      var de = (J) => {
        var ne = wa(), ge = c(ne);
        Ye(ge, { ariaLabel: "Select Column: Options", get checked() {
          return r()[r().length - 1];
        }, set checked(Q) {
          r(r()[r().length - 1] = Q, true);
        }, children: (Q, I) => {
          ot();
          var q = nt("Options");
          a(Q, q);
        }, $$slots: { default: true } }), s(ne), a(J, ne);
      };
      U(re, (J) => {
        e.options && J(de);
      });
    }
    s(B), a(f, B);
  }, $$slots: { button: true, default: true } });
  var it = z(wt, 2), zt = c(it);
  De(zt, () => e.caption ?? at), s(it);
  var Mt = z(it, 2);
  {
    var At = (v) => {
      var f = ka();
      a(v, f);
    }, Et = (v) => {
      Ar(v, { get items() {
        return e.rows;
      }, get compact() {
        return d();
      }, get itemsPaginated() {
        return t(E);
      }, set itemsPaginated(f) {
        y(E, f, true);
      }, get page() {
        return t(i);
      }, set page(f) {
        y(i, f, true);
      }, get pageSize() {
        return t(b);
      }, set pageSize(f) {
        y(b, f, true);
      } });
    };
    U(Mt, (v) => {
      t(Z) ? v(At) : v(Et, false);
    });
  }
  s(bt), s(_t), s(X), L((v, f) => {
    h(X, "aria-colcount", u.length), h(X, "aria-rowcount", t(H)), je = Ae(X, "", je, v), Ie = Ae(Xe, "", Ie, f);
  }, [() => ({ width: e.width, "max-width": e.maxWidth }), () => ({ "grid-template-columns": w() })]), a(n, X), be();
}
Ee(["mousedown"]);
var pa = k("<p>no results</p>");
function Ca(n, e) {
  _e(e, true);
  let r = A(oe([])), d = A(oe([]));
  ve(() => {
    let i = [], b = [];
    if (e.rows.length > 0) {
      for (let O of e.rows[0].columns) i.push({ content: O.name, initialWidth: "12rem", orderType: o(O.value) });
      for (let O of e.rows) {
        let C = [];
        for (let N of O.columns) C.push({ content: x(N.value) });
        b.push(C);
      }
    }
    y(r, i, true), y(d, b, true);
  });
  function l(i) {
    return [...new Uint8Array(i)].map((b) => b.toString(16).padStart(2, "0")).join("");
  }
  function o(i) {
    return i.hasOwnProperty("Integer") || i.hasOwnProperty("Real") ? "number" : "string";
  }
  function x(i) {
    return i.hasOwnProperty("Integer") ? i.Integer : i.hasOwnProperty("Real") ? i.Real : i.hasOwnProperty("Text") ? i.Text : i.hasOwnProperty("Blob") ? `x'${l(i.Blob)}'` : "NULL";
  }
  var m = fe(), S = ee(m);
  {
    var u = (i) => {
      xa(i, { get columns() {
        return t(r);
      }, paginationPageSize: 100, get rows() {
        return t(d);
      }, set rows(b) {
        y(d, b, true);
      } });
    }, M = (i) => {
      var b = pa();
      a(i, b);
    };
    U(S, (i) => {
      t(r).length > 0 && t(d).length > 0 ? i(u) : i(M, false);
    });
  }
  a(n, m), be();
}
function Sa(n, e) {
  n.ctrlKey && n.code === "Enter" && e();
}
var Pa = k('<div role="textbox" tabindex="0" class="query svelte-1o8x9h5" contenteditable=""></div>'), La = k('<div class="err"> </div>'), Ta = k('<!> <!> <div id="query-results" class="svelte-1o8x9h5"><!></div>', 1);
function Da(n, e) {
  _e(e, true);
  let r = _(e, "query", 7), d = A(oe([])), l = A(""), o = A(void 0), x = A(void 0), m = le(() => t(o) && t(x) ? `${t(o) - t(x)}px` : "100%");
  ve(() => {
    r().query.startsWith(yt) && (r().query = r().query.replace(`${yt}
`, ""), S());
  });
  async function S() {
    y(d, [], true), y(l, "");
    let D = [];
    for (let Z of r().query.split(/\r?\n/)) Z.startsWith("--") || D.push(Z);
    let K = D.join(`
`), E = await Xt("/query", K);
    if (E.status === 200) y(d, await E.json(), true);
    else {
      let Z = await E.json();
      y(l, Object.values(Z)[0], true);
    }
  }
  async function u(D) {
    y(x, D, true);
  }
  var M = Ta(), i = ee(M);
  Kt(i, { resizeBottom: true, minHeightPx: 100, initialHeightPx: 300, onResizeBottom: u, children: (D, K) => {
    var E = Pa();
    E.__keydown = [Sa, S], or("innerText", E, () => r().query, (Z) => r().query = Z), a(D, E);
  }, $$slots: { default: true } });
  var b = z(i, 2);
  {
    var O = (D) => {
      var K = La(), E = c(K, true);
      s(K), L(() => se(E, t(l))), a(D, K);
    };
    U(b, (D) => {
      t(l) && D(O);
    });
  }
  var C = z(b, 2);
  let N;
  var R = c(C);
  Ca(R, { get rows() {
    return t(d);
  }, set rows(D) {
    y(d, D, true);
  } }), s(C), L((D) => N = Ae(C, "", N, D), [() => ({ height: t(m), "max-height": t(m) })]), nr("innerHeight", (D) => y(o, D, true)), a(n, M), be();
}
Ee(["keydown"]);
var Ia = ye('<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12"></path></svg>');
function Ba(n, e) {
  let r = _(e, "color", 8, "var(--col-err)"), d = _(e, "opacity", 8, 0.9), l = _(e, "width", 8, "1.5rem");
  var o = Ia();
  h(o, "stroke-width", 2), L(() => {
    h(o, "width", l()), h(o, "color", r()), h(o, "opacity", d());
  }), a(n, o);
}
function Oa(n, e, r, d) {
  t(e) ? n.code === "Enter" && (n.preventDefault(), r()) : d();
}
function pt(n, e, r) {
  e.onClose(r());
}
var Ra = k('<div class="row svelte-1ml8s23"><div role="button" tabindex="0"><!></div> <div class="close svelte-1ml8s23"><div role="button" tabindex="0" class="close-inner svelte-1ml8s23"><!></div></div></div>');
function za(n, e) {
  _e(e, true);
  let r = _(e, "tab", 15), d = _(e, "tabSelected", 15), l, o = le(() => d() === r());
  function x() {
    let C = l.innerText;
    r(C), d(C);
  }
  function m() {
    t(o) || d(r());
  }
  var S = Ra(), u = c(S);
  u.__click = m, u.__keydown = [Oa, o, x, m];
  var M = c(u);
  De(M, () => e.children), s(u), Ne(u, (C) => l = C, () => l);
  var i = z(u, 2), b = c(i);
  b.__click = [pt, e, r], b.__keydown = [pt, e, r];
  var O = c(b);
  Ba(O, { color: "hsl(var(--error))", width: "1.2rem" }), s(b), s(i), s(S), L(() => {
    Ce(u, 1, Jt(t(o) ? "tab selected" : "tab"), "svelte-1ml8s23"), h(u, "contenteditable", t(o));
  }), ht("blur", u, x), a(n, S), be();
}
Ee(["click", "keydown"]);
var Ma = ye('<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15"></path></svg>');
function Aa(n, e) {
  let r = _(e, "opacity", 8, 0.9), d = _(e, "width", 8, "1.5rem");
  var l = Ma();
  h(l, "stroke-width", 2), L(() => {
    h(l, "width", d()), h(l, "opacity", r());
  }), a(n, l);
}
function Ct() {
  ae.push({ id: Ke(6), query: $t });
}
var Ea = k('<div id="tabs" class="svelte-ko98zn"><!> <div role="button" tabindex="0" title="Add New Tab" class="ctrl add-new svelte-ko98zn"><!></div></div> <!>', 1);
function ja(n, e) {
  _e(e, true);
  let r = A(oe(ae[0].id)), d = le(() => ae.filter((i) => i.id === t(r))[0]);
  ve(() => {
    ae.length > 0 ? y(r, ae[ae.length - 1].id, true) : y(r, "");
  });
  function l(i) {
    let O = ae.map((C) => C.id).indexOf(i);
    t(r) === i ? ae.length === 1 ? (ae.push(er), ae.shift(), y(r, ae[0].id, true)) : O === 0 ? (ae.shift(), y(r, ae[0].id, true)) : (ae.splice(O, 1), y(r, ae[O - 1].id, true)) : ae.splice(O, 1);
  }
  var o = Ea(), x = ee(o), m = c(x);
  Se(m, 17, () => ae, (i) => i.id, (i, b, O) => {
    za(i, { onClose: l, get tab() {
      return t(b).id;
    }, set tab(C) {
      t(b).id = C;
    }, get tabSelected() {
      return t(r);
    }, set tabSelected(C) {
      y(r, C, true);
    }, children: (C, N) => {
      ot();
      var R = nt();
      L(() => se(R, t(b).id)), a(C, R);
    }, $$slots: { default: true } });
  });
  var S = z(m, 2);
  S.__click = [Ct], S.__keydown = [Ct];
  var u = c(S);
  Aa(u, {}), s(S), s(x);
  var M = z(x, 2);
  Da(M, { get query() {
    return t(d);
  } }), a(n, o), be();
}
Ee(["click", "keydown"]);
var Ua = k('<meta property="description" content="Hiqlite Dashboard"/>');
function Fa(n) {
  Qt((e) => {
    var r = Ua();
    Nt.title = "Hiqlite", a(e, r);
  }), ja(n, {});
}
export {
  Fa as component
};
