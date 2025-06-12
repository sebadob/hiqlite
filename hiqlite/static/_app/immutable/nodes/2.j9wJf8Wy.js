import { r as qt, w as Ht, f as k, a, c as ue, p as ht, o as ye, d as Ee, s as se, t as at, q as Wt } from "../chunks/C_iGe9Tc.js";
import "../chunks/C0SPxqjE.js";
import { F as Qt, p as _e, k as c, l as i, t as L, g as t, a6 as le, j as be, a as ve, a4 as A, i as ee, s as z, O as y, X as oe, b as Xe, I as ot, aT as nt, aS as Nt } from "../chunks/CxznHt52.js";
import { s as Ie, a as f, j as Ce, m as Ge, B as Pe, r as St, b as Ae, d as Ft, g as Se, i as Te, o as Vt, p as Zt, q as Yt, I as Xt, R as Gt, n as yt, u as Kt, k as Jt, Q as ae, v as $t, D as er } from "../chunks/pNsTmdlU.js";
import { p as _, s as tr, a as rr, i as U, b as Ne } from "../chunks/DQ-LUyGE.js";
import { s as ar } from "../chunks/ttXnxlq3.js";
function or(n, e, r, d = r) {
  e.addEventListener("input", () => {
    d(e[n]);
  }), Qt(() => {
    var l = r();
    if (e[n] !== l) if (l == null) {
      var o = e[n];
      d(o);
    } else e[n] = l + "";
  });
}
function nr(n, e) {
  qt(window, ["resize"], () => Ht(() => e(window[n])));
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
  var s = c(u);
  Ie(s, () => e.children), i(u), i(S), L((b) => {
    f(u, "href", e.href), f(u, "target", e.target), f(u, "aria-current", t(m)), M = Ce(u, 1, "svelte-a0xtvp", null, M, b);
  }, [() => ({ hideUnderline: x() })]), a(n, S), be(), d();
}
var vr = k('<!> <div class="popover svelte-1au8ouo" popover="auto"><div class="inner fade-in svelte-1au8ouo"><!></div></div>', 1);
function lt(n, e) {
  _e(e, true);
  let r = _(e, "ref", 15), d = _(e, "roleButton", 3, "button"), l = _(e, "offsetLeft", 3, "0px"), o = _(e, "offsetTop", 3, "0px"), x = _(e, "close", 15);
  const m = Ge(8), S = Ge(8);
  let u = A(void 0), M = A(false);
  ve(() => {
    x(b);
  });
  function s() {
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
  function B(H) {
    var _a2;
    let Z = H.newState;
    y(M, Z === "open"), (_a2 = e.onToggle) == null ? void 0 : _a2.call(e, Z);
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
  }, onclick: s, get invisible() {
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
  }, children: (H, Z) => {
    var te = ue(), p = ee(te);
    Ie(p, () => e.button), a(H, te);
  }, $$slots: { default: true } });
  var O = z(N, 2), R = c(O), G = c(R);
  {
    var E = (H) => {
      var Z = ue(), te = ee(Z);
      {
        var p = (g) => {
          var P = ue(), w = ee(P);
          Ie(w, () => e.children), a(g, P);
        };
        U(te, (g) => {
          t(M) && g(p);
        });
      }
      a(H, Z);
    }, F = (H) => {
      var Z = ue(), te = ee(Z);
      Ie(te, () => e.children), a(H, Z);
    };
    U(G, (H) => {
      e.lazy ? H(E) : H(F, false);
    });
  }
  i(R), i(O), Ne(O, (H) => y(u, H), () => t(u)), L(() => {
    f(O, "id", S), f(O, "aria-label", e.ariaLabel), f(O, "aria-labelledby", m);
  }), ht("toggle", O, B), a(n, C), be();
}
var cr = ye('<svg fill="none" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M12 9.75L14.25 12m0 0l2.25 2.25M14.25 12l2.25-2.25M14.25 12L12 14.25m-2.58 4.92l-6.375-6.375a1.125 1.125 0 010-1.59L9.42 4.83c.211-.211.498-.33.796-.33H19.5a2.25 2.25 0 012.25 2.25v10.5a2.25 2.25 0 01-2.25 2.25h-9.284c-.298 0-.585-.119-.796-.33z"></path></svg>');
function dr(n, e) {
  let r = _(e, "color", 8, "currentColor"), d = _(e, "opacity", 8, 0.9), l = _(e, "width", 8, "1.5rem");
  var o = cr();
  f(o, "stroke-width", 2), L(() => {
    f(o, "stroke", r()), f(o, "width", l()), f(o, "opacity", d());
  }), a(n, o);
}
var ur = ye('<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M21 21l-5.197-5.197m0 0A7.5 7.5 0 105.196 5.196a7.5 7.5 0 0010.607 10.607z"></path></svg>');
function fr(n, e) {
  let r = _(e, "opacity", 8, 0.9), d = _(e, "width", 8, "1.5rem");
  var l = ur();
  f(l, "stroke-width", 2), L(() => {
    f(l, "width", d()), f(l, "opacity", r());
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
  const x = Ge(8), m = Ge(8);
  let S = A(void 0), u = le(() => e.datalist && e.datalist.length > 0 ? m : void 0);
  ve(() => {
    l(s);
  });
  function M() {
    var _a2;
    (_a2 = e.onSearch) == null ? void 0 : _a2.call(e, r());
  }
  function s() {
    var _a2;
    (_a2 = t(S)) == null ? void 0 : _a2.focus();
  }
  var b = kr();
  let B;
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
      } }), i(g), a(p, g);
    };
    U(C, (p) => {
      e.options && p(N);
    });
  }
  var O = z(C, 2);
  St(O), O.__keydown = [hr, M, e, r], Ne(O, (p) => y(S, p), () => t(S));
  var R = z(O, 2);
  {
    var G = (p) => {
      var g = br();
      Se(g, 21, () => e.datalist, Te, (P, w, W, j) => {
        var V = _r(), fe = {};
        L(() => {
          fe !== (fe = t(w)) && (V.value = (V.__value = t(w)) ?? "");
        }), a(P, V);
      }), i(g), L(() => f(g, "id", m)), a(p, g);
    };
    U(R, (p) => {
      e.datalist && p(G);
    });
  }
  var E = z(R, 2), F = c(E), H = c(F);
  Pe(H, { ariaLabel: "Delete Search Input", invisible: true, onclick: () => r(""), children: (p, g) => {
    dr(p, { color: "hsl(var(--bg-high))", width: 24 });
  }, $$slots: { default: true } }), i(F), i(E);
  var Z = z(E, 2);
  {
    var te = (p) => {
      var g = mr(), P = c(g);
      Pe(P, { ariaLabel: "Search", invisible: true, onclick: M, children: (w, W) => {
        var j = wr(), V = c(j);
        fr(V, {}), i(j), a(w, j);
      }, $$slots: { default: true } }), i(g), a(p, g);
    };
    U(Z, (p) => {
      e.onSearch && p(te);
    });
  }
  i(b), L(() => {
    B = Ae(b, "", B, { border: e.borderless ? void 0 : "1px solid hsl(var(--bg-high))", width: o() }), f(O, "id", x), f(O, "list", t(u));
  }), ht("focus", O, () => {
    var _a2;
    return (_a2 = e.onFocus) == null ? void 0 : _a2.call(e);
  }), Ft(O, r), a(n, b), be();
}
Ee(["keydown"]);
var xr = ye('<svg fill="none" viewBox="0 0 24 24" color="currentColor" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M8.25 4.5l7.5 7.5-7.5 7.5"></path></svg>');
function ft(n, e) {
  let r = _(e, "opacity", 8, 0.9), d = _(e, "width", 8, "1.5rem");
  var l = xr();
  f(l, "stroke-width", 2), L(() => {
    f(l, "width", d()), f(l, "opacity", r());
  }), a(n, l);
}
function pr(n, e, r, d, l) {
  let o = n.code;
  o === "ArrowDown" ? (n.preventDefault(), e() && y(r, t(r) + 1)) : o === "ArrowUp" ? (n.preventDefault(), e() && y(r, t(r) - 1)) : o === "Enter" && t(r) > -1 ? d(t(l)[t(r)]) : o === "Enter" && t(r) === -1 && t(l).length === 1 && d(t(l)[0]);
}
var Cr = k('<div class="btn svelte-1j5gmms"> <div class="chevron svelte-1j5gmms"><!></div></div>'), Sr = k('<div class="optPopover svelte-1j5gmms"> </div>'), Pr = k('<div role="listbox" tabindex="0" class="popover svelte-1j5gmms"><!> <div class="popoverOptions svelte-1j5gmms"></div></div>'), Lr = k('<option class="opt svelte-1j5gmms"> </option>'), Tr = k("<select></select>");
function Pt(n, e) {
  _e(e, true);
  let r = _(e, "ref", 15), d = _(e, "options", 19, () => []), l = _(e, "value", 15), o = _(e, "asPopover", 3, true), x = _(e, "borderless", 3, false), m = _(e, "withSearch", 3, false), S = _(e, "fallbackOptions", 3, false), u = A(void 0), M = A(oe(S() ? false : o())), s = A(void 0), b = A(oe(m() ? -1 : 0)), B = A(void 0), C = A(""), N = le(() => {
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
        y(b, -1), (_b = t(B)) == null ? void 0 : _b();
        return;
      }
    } else t(b) < 0 ? y(b, t(N).length - 1) : t(b) > t(N).length - 1 && y(b, 0), O();
  });
  function O() {
    if (t(u)) {
      let p = t(u).getElementsByTagName("button")[t(b)];
      p.scrollIntoView({ behavior: "smooth", block: "center" }), p.focus();
    } else console.error("refOptions is undefined");
  }
  function R(p) {
    var _a2;
    p === "open" && (m() ? (y(b, -1), (_a2 = t(B)) == null ? void 0 : _a2()) : (y(b, d().findIndex((g) => g === l()) || 0, true), O()));
  }
  function G() {
    return t(N).length > 0 ? true : (y(b, -1), false);
  }
  function E(p) {
    l(p), y(C, ""), setTimeout(() => {
      var _a2;
      (_a2 = t(s)) == null ? void 0 : _a2();
    }, 20);
  }
  var F = ue(), H = ee(F);
  {
    var Z = (p) => {
      lt(p, { get ariaLabel() {
        return e.ariaLabel;
      }, roleButton: "combobox", btnInvisible: true, get offsetTop() {
        return e.offsetTop;
      }, get offsetLeft() {
        return e.offsetLeft;
      }, onToggle: R, get onLeft() {
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
        return t(s);
      }, set close(P) {
        y(s, P, true);
      }, button: (P) => {
        var w = Cr(), W = c(w), j = z(W), V = c(j);
        ft(V, { width: 14 }), i(j), i(w), L(() => {
          f(w, "data-border", !x()), se(W, `${l() ?? ""} `);
        }), a(P, w);
      }, children: (P, w) => {
        var W = Pr();
        W.__keydown = [pr, G, b, E, N];
        let j;
        var V = c(W);
        {
          var fe = (he) => {
            yr(he, { onFocus: () => y(b, -1), get value() {
              return t(C);
            }, set value(K) {
              y(C, K, true);
            }, get focus() {
              return t(B);
            }, set focus(K) {
              y(B, K, true);
            } });
          };
          U(V, (he) => {
            m() && he(fe);
          });
        }
        var xe = z(V, 2);
        Se(xe, 21, () => t(N), Te, (he, K, je) => {
          Pe(he, { invisible: true, invisibleOutline: true, onclick: () => E(t(K)), children: (Fe, Ke) => {
            var De = Sr(), Je = c(De, true);
            i(De), L(() => {
              f(De, "aria-selected", l() === t(K)), f(De, "data-focus", t(b) === je), se(Je, t(K));
            }), a(Fe, De);
          }, $$slots: { default: true } });
        }), i(xe), Ne(xe, (he) => y(u, he), () => t(u)), i(W), L(() => j = Ae(W, "", j, { "max-height": e.maxHeight })), a(P, W);
      }, $$slots: { button: true, default: true } });
    }, te = (p) => {
      var g = Tr();
      let P;
      Se(g, 21, () => t(N), Te, (w, W) => {
        var j = Lr(), V = {}, fe = c(j, true);
        i(j), L(() => {
          V !== (V = t(W)) && (j.value = (j.__value = t(W)) ?? ""), Vt(j, l() === t(W)), se(fe, t(W));
        }), a(w, j);
      }), i(g), L((w) => {
        f(g, "name", e.name), f(g, "aria-label", e.ariaLabel), P = Ce(g, 1, "svelte-1j5gmms", null, P, w);
      }, [() => ({ borderless: x() })]), Zt(g, l), a(p, g);
    };
    U(H, (p) => {
      t(M) ? p(Z) : p(te, false);
    });
  }
  a(n, F), be();
}
Ee(["keydown"]);
var Ir = k('<div class="link noselect svelte-1bye1t3"> </div>'), Dr = k('<li class="svelte-1bye1t3"><!></li>'), Br = k('<nav aria-label="Pagination" class="svelte-1bye1t3"><ul class="svelte-1bye1t3"></ul></nav>'), Or = k('<div class="flex gap-10 svelte-1bye1t3"><div class="flex gap-05 chunkSize noselect svelte-1bye1t3"><div class="svelte-1bye1t3">Entries</div> <div class="svelte-1bye1t3"><!></div></div> <div class="font-label total svelte-1bye1t3"> </div></div>'), Rr = k('<div class="iconLeft svelte-1bye1t3" aria-label="Go to previous page"><!></div>'), zr = k('<div class="iconRight svelte-1bye1t3" aria-label="Go to next page"><!></div>'), Mr = k('<div class="container svelte-1bye1t3"><!> <!> <!> <!></div>');
function Ar(n, e) {
  _e(e, true);
  const r = (g) => {
    var P = Br(), w = c(P);
    Se(w, 21, () => t(b), Te, (W, j) => {
      var V = Dr(), fe = c(V);
      Pe(fe, { invisible: true, onclick: () => N(t(j)), onLeft: B, onRight: C, children: (xe, he) => {
        var K = Ir(), je = c(K, true);
        i(K), L(() => se(je, t(j))), a(xe, K);
      }, $$slots: { default: true } }), i(V), L(() => {
        f(V, "aria-label", `go to page number: ${t(j)}`), f(V, "aria-current", x() === t(j) ? "step" : void 0);
      }), a(W, V);
    }), i(w), i(P), a(g, P);
  }, d = (g) => {
    var P = Or(), w = c(P), W = z(c(w), 2), j = c(W);
    Pt(j, { ariaLabel: "Page Count", get options() {
      return l;
    }, offsetTop: "-17rem", borderless: true, get value() {
      return m();
    }, set value(xe) {
      m(xe);
    } }), i(W), i(w);
    var V = z(w, 2), fe = c(V);
    i(V), i(P), L(() => se(fe, `Total: ${e.items.length ?? ""}`)), a(g, P);
  }, l = [5, 7, 10, 15, 20, 30, 50, 100];
  let o = _(e, "itemsPaginated", 15), x = _(e, "page", 15, 1), m = _(e, "pageSize", 31, () => oe(l[0])), S = _(e, "compact", 3, false);
  const u = 16;
  let M = Xe(() => m()), s = A(oe([])), b = A(oe([]));
  ve(() => {
    m() !== M && (M = Xe(() => m()), x(1));
  }), ve(() => {
    let g = [];
    for (let P = 0; P < e.items.length; P += m()) {
      const w = e.items.slice(P, P + m());
      g.push(w);
    }
    y(s, g, true), o(g[x() - 1]);
  }), ve(() => {
    O();
  });
  function B() {
    x() > 1 && N(x() - 1);
  }
  function C() {
    x() < t(s).length && N(x() + 1);
  }
  function N(g) {
    x(g), O();
  }
  function O() {
    let g = [], P = Math.floor(m() / 2);
    if (t(s).length <= m()) for (let w = 1; w <= t(s).length; w++) g.push(w);
    else if (x() <= P) for (let w = 1; w <= m(); w++) g.push(w);
    else if (x() > t(s).length - P - 1) for (let w = t(s).length - m(); w <= t(s).length - 1; w++) g.push(w + 1);
    else for (let w = x() - P; w < x() - P + m(); w++) g.push(w);
    y(b, g, true);
  }
  var R = Mr(), G = c(R);
  const E = le(() => x() === 1);
  Pe(G, { onclick: B, invisible: true, get isDisabled() {
    return t(E);
  }, children: (g, P) => {
    var w = Rr(), W = c(w);
    ft(W, { width: u }), i(w), L(() => f(w, "data-disabled", x() === 1)), a(g, w);
  }, $$slots: { default: true } });
  var F = z(G, 2);
  r(F);
  var H = z(F, 2);
  const Z = le(() => x() === t(s).length);
  Pe(H, { onclick: C, invisible: true, get isDisabled() {
    return t(Z);
  }, children: (g, P) => {
    var w = zr(), W = c(w);
    ft(W, { width: u }), i(w), L(() => f(w, "data-disabled", x() === t(s).length)), a(g, w);
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
  i(R), a(n, R), be();
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
  Ie(M, () => e.children ?? ot), i(u), i(x), L(() => {
    f(m, "name", e.name), m.disabled = e.disabled, f(m, "aria-disabled", e.disabled), f(m, "aria-checked", r()), f(m, "aria-label", d()), S = Ae(m, "", S, { "border-color": l() });
  }), Yt(m, r), a(n, x), be();
}
Ee(["click", "keydown"]);
var Ur = ye('<svg fill="none" viewBox="0 0 24 24" stroke="currentColor" color="hsl(var(--action))"><path stroke-linecap="round" stroke-linejoin="round" d="M4.5 12.75l6 6 9-13.5"></path></svg>'), qr = ye('<svg fill="none" viewBox="0 0 24 24" stroke="currentColor" color="hsl(var(--error))"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12"></path></svg>');
function Hr(n, e) {
  _e(e, true);
  const r = 20, d = 0.9;
  var l = ue(), o = ee(l);
  {
    var x = (S) => {
      var u = Ur();
      f(u, "stroke-width", 2), f(u, "width", r), f(u, "opacity", d), a(S, u);
    }, m = (S) => {
      var u = qr();
      f(u, "stroke-width", 2), f(u, "width", r), f(u, "opacity", d), a(S, u);
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
  f(o, "stroke-width", 2), L(() => {
    f(o, "width", l()), f(o, "color", r()), f(o, "opacity", d());
  }), a(n, o);
}
var Nr = ye('<svg fill="none" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M3 7.5 7.5 3m0 0L12 7.5M7.5 3v13.5m13.5 0L16.5 21m0 0L12 16.5m4.5 4.5V7.5"></path></svg>');
function Fr(n, e) {
  let r = _(e, "color", 8, "currentColor"), d = _(e, "opacity", 8, 0.9), l = _(e, "width", 8, "1.5rem");
  var o = Nr();
  f(o, "stroke-width", 2), L(() => {
    f(o, "stroke", r()), f(o, "width", l()), f(o, "opacity", d());
  }), a(n, o);
}
var Vr = ye(`<svg fill="none" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213
            1.281c.063.374.313.686.645.87.074.04.147.083.22.127.325.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 0
            1 1.37.49l1.296 2.247a1.125 1.125 0 0 1-.26 1.431l-1.003.827c-.293.241-.438.613-.43.992a7.723 7.723 0 0
            1 0 .255c-.008.378.137.75.43.991l1.004.827c.424.35.534.955.26 1.43l-1.298 2.247a1.125 1.125 0 0
            1-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.47 6.47 0 0
            1-.22.128c-.331.183-.581.495-.644.869l-.213 1.281c-.09.543-.56.94-1.11.94h-2.594c-.55 0-1.019-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52
            0 0 1-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 0 1-1.369-.49l-1.297-2.247a1.125
            1.125 0 0 1 .26-1.431l1.004-.827c.292-.24.437-.613.43-.991a6.932 6.932 0 0 1 0-.255c.007-.38-.138-.751-.43-.992l-1.004-.827a1.125 1.125
            0 0 1-.26-1.43l1.297-2.247a1.125 1.125 0 0 1 1.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.086.22-.128.332-.183.582-.495.644-.869l.214-1.28Z"></path><path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z"></path></svg>`);
function Zr(n, e) {
  let r = _(e, "color", 8, "currentColor"), d = _(e, "opacity", 8, 0.9), l = _(e, "width", 8, "1.5rem");
  var o = Vr();
  f(o, "stroke-width", 2), L(() => {
    f(o, "stroke", r()), f(o, "width", l()), f(o, "opacity", d());
  }), a(n, o);
}
async function Yr(n) {
  var _a2;
  await ((_a2 = navigator == null ? void 0 : navigator.clipboard) == null ? void 0 : _a2.writeText(n));
}
var Xr = ye(`<svg fill="none" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M6.75 12a.75.75 0 1 1-1.5 0 .75.75 0 0 1 1.5 0ZM12.75 12a.75.75 0 1 1-1.5 0 .75.75 0 0 1 1.5
            0ZM18.75 12a.75.75 0 1 1-1.5 0 .75.75 0 0 1 1.5 0Z"></path></svg>`);
function Gr(n, e) {
  let r = _(e, "color", 8, "currentColor"), d = _(e, "opacity", 8, 0.9), l = _(e, "width", 8, "1.5rem");
  var o = Xr();
  f(o, "stroke-width", 2), L(() => {
    f(o, "stroke", r()), f(o, "width", l()), f(o, "opacity", d());
  }), a(n, o);
}
var Kr = k('<span class="btnSelect svelte-12u4ifk"><!></span>'), Jr = k('<th class="headerCheckbox svelte-12u4ifk"><!> <!></th>'), $r = k('<span class="iconOrder svelte-12u4ifk"><!></span>'), ea = k('<span class="orderText svelte-12u4ifk"> </span> <!>', 1), ta = k('<span class="rawText svelte-12u4ifk"> </span>'), ra = k('<th class="svelte-12u4ifk"><span class="flex-1 label svelte-12u4ifk"><!></span> <span class="relative"><span role="none" class="absolute sizeable svelte-12u4ifk"></span></span></th>'), aa = k('<th class="headerOptions svelte-12u4ifk"><!></th>'), oa = k('<td class="checkbox svelte-12u4ifk"><!></td>'), na = k("<span> </span>"), la = k("<span> </span>"), ia = k("<span> </span>"), sa = k("<span><!></span>"), va = k("<span> </span>"), ca = k("<span> </span>"), da = k('<td class="svelte-12u4ifk"><!></td>'), ua = k('<span class="btnOptions svelte-12u4ifk"><!></span>'), fa = k('<td class="options svelte-12u4ifk"><!></td>'), ha = k("<tr><!><!><!></tr>"), ga = k('<div class="eye svelte-12u4ifk"><!></div>'), _a = k('<div class="columnSelect svelte-12u4ifk"><!></div>'), ba = k('<div class="columnSelect svelte-12u4ifk"><!></div>'), wa = k('<div class="columnSelect svelte-12u4ifk"><!></div>'), ma = k('<div class="columnsSelect svelte-12u4ifk"><!> <!> <!></div>'), ka = k("<div></div>"), ya = k('<table class="svelte-12u4ifk"><thead class="svelte-12u4ifk"><tr class="svelte-12u4ifk"><!><!><!></tr></thead><tbody class="svelte-12u4ifk"><!></tbody><caption class="flex space-between svelte-12u4ifk"><div class="flex"><!> <div class="caption svelte-12u4ifk"><!></div> <!></div></caption></table>');
function xa(n, e) {
  _e(e, true);
  let r = _(e, "showColumns", 31, () => oe(Array(e.columns.length).fill(true))), d = _(e, "paginationCompact", 3, false), l = _(e, "paginationPageSize", 3, 15), o = _(e, "selectInitHide", 3, false), x = _(e, "minWidthColPx", 3, 50);
  const m = "3rem", S = "2rem";
  let u = oe(P()), M = A(oe(Xe(() => u))), s = A(1), b = A(oe(Xe(() => l()))), B = A(false), C = A(oe(Array(e.rows.length).fill(false))), N = le(() => t(C).find((v) => v === true)), O = A(void 0), R = A(oe([])), G = A("up"), E = A(oe([])), F = le(() => e.paginationDisabled !== void 0 ? e.paginationDisabled : e.rows.length < l()), H = le(() => e.paginationDisabled || t(F) ? e.rows.length : t(E) && t(E).length ? t(E).length : (t(s) != 1 && y(s, 1), 0)), Z = oe(Array(Xe(() => u.length)).fill(void 0)), te = A(void 0), p = 0, g = A(void 0);
  ve(() => {
    setTimeout(() => {
      for (let v = 1; v < u.length; v++) if (u[v] === "auto") {
        p = e.select ? v : v - 1;
        let h = Z[v];
        h && he(h.getBoundingClientRect().width);
      }
    }, 150);
  }), ve(() => {
    let v = Array(e.rows.length).fill(false);
    if (t(B)) {
      let h;
      t(s) === 1 ? h = 0 : h = (t(s) - 1) * t(b);
      let T = Math.min(t(s) * t(b), e.rows.length);
      for (let D = h; D < T; D++) v[D] = true;
    }
    y(C, v, true);
  }), ve(() => {
    let v = e.paginationDisabled || t(F) ? e.rows.length : t(E).length || 0;
    y(R, Array(v).fill(() => console.error("un-initialized popover close option")), true);
  }), ve(() => {
    let v = [];
    for (let h = 0; h < u.length; h++) r()[h] && v.push(u[h]);
    y(M, v, true);
  }), ve(() => {
    if (e.highlight !== void 0 && t(te)) {
      let v = e.highlight;
      !t(F) && t(s) > 1 && (v = e.highlight - (t(s) - 1) * t(b)), y(g, v, true), setTimeout(() => {
        var _a2, _b;
        (_b = (_a2 = t(te)) == null ? void 0 : _a2.getElementsByClassName("highlight")[0]) == null ? void 0 : _b.scrollIntoView({ behavior: "smooth", block: "center" });
      }, 250);
    } else y(g, void 0);
  });
  function P() {
    let v = e.columns.map((T) => T.initialWidth), h = [...r()];
    return e.select && (v = [m, ...v], h = [!o(), ...h]), e.options && (v = [...v, S], h = [...h, true]), r(h), v;
  }
  function w() {
    return t(M).join(" ");
  }
  function W(v, h) {
    y(C, Array(e.rows.length).fill(false), true);
    let T = 1;
    t(G) === "up" ? (T = -1, y(G, "down")) : y(G, "up"), h === "string" ? e.rows.sort((D, Y) => D[v].content.localeCompare(Y[v].content) * T) : h === "number" && e.rows.sort((D, Y) => (D[v].content - Y[v].content) * T);
  }
  function j(v) {
    return !t(F) && t(s) > 1 ? (t(s) - 1) * t(b) + v : v;
  }
  function V(v) {
    p = v;
    let h = Z[v];
    h ? (he(h.getBoundingClientRect().width), window.addEventListener("mousemove", xe), window.addEventListener("mouseup", fe, { once: true })) : console.error("invalid ref from refCols in onMouseDown");
  }
  function fe() {
    window.removeEventListener("mousemove", xe);
  }
  function xe(v) {
    let h = Z[p];
    if (h) {
      let T = h.getBoundingClientRect().left, D = window.scrollX + v.x - T;
      he(D);
    } else console.error("invalid ref from refCols in onMove");
  }
  function he(v) {
    v = Math.ceil(v), v < x() && (v = x()), u[e.select ? p + 1 : p] = `${v}px`;
  }
  var K = ya();
  let je;
  var Fe = c(K), Ke = c(Fe);
  let De;
  var Je = c(Ke);
  {
    var Lt = (v) => {
      var h = Jr(), T = c(h);
      Ye(T, { ariaLabel: "Select All", borderColor: "hsla(var(--text), .4)", get checked() {
        return t(B);
      }, set checked(ie) {
        y(B, ie, true);
      } });
      var D = z(T, 2);
      const Y = le(() => !t(N));
      lt(D, { ariaLabel: "Selected Options", get btnDisabled() {
        return t(Y);
      }, btnInvisible: true, get close() {
        return t(O);
      }, set close(X) {
        y(O, X, true);
      }, button: (X) => {
        var re = Kr(), ce = c(re);
        Qr(ce, { width: "1rem" }), i(re), L(() => f(re, "data-disabled", !t(N))), a(X, re);
      }, children: (X, re) => {
        var ce = ue(), J = ee(ce);
        Ie(J, () => e.select, () => t(C), () => t(O)), a(X, ce);
      }, $$slots: { button: true, default: true } }), i(h), a(v, h);
    };
    U(Je, (v) => {
      e.select && r()[0] && v(Lt);
    });
  }
  var gt = z(Je);
  Se(gt, 17, () => e.columns, Te, (v, h, T) => {
    var D = ue(), Y = ee(D);
    {
      var ie = (X) => {
        var re = ra(), ce = c(re), J = c(ce);
        {
          var ne = (q) => {
            var $ = ea(), de = ee($), we = c(de, true);
            i(de);
            var pe = z(de, 2);
            Pe(pe, { invisible: true, onclick: () => W(T, t(h).orderType), children: (Me, mt) => {
              var Ve = $r(), st = c(Ve);
              Fr(st, { width: "1rem" }), i(Ve), a(Me, Ve);
            }, $$slots: { default: true } }), L(() => se(we, t(h).content)), a(q, $);
          }, ge = (q) => {
            var $ = ta(), de = c($, true);
            i($), L(() => se(de, t(h).content)), a(q, $);
          };
          U(J, (q) => {
            t(h).orderType ? q(ne) : q(ge, false);
          });
        }
        i(ce);
        var Q = z(ce, 2), I = c(Q);
        I.__mousedown = () => V(T), i(Q), i(re), Ne(re, (q, $) => Z[$] = q, (q) => Z == null ? void 0 : Z[q], () => [T]), a(X, re);
      };
      U(Y, (X) => {
        r()[e.select ? T + 1 : T] && X(ie);
      });
    }
    a(v, D);
  });
  var Tt = z(gt);
  {
    var It = (v) => {
      var h = aa(), T = c(h);
      Zr(T, { width: "1.2rem" }), i(h), a(v, h);
    };
    U(Tt, (v) => {
      e.options && r()[r().length - 1] && v(It);
    });
  }
  i(Ke), i(Fe);
  var $e = z(Fe);
  {
    const v = (h, T = ot, D = ot) => {
      var Y = ha();
      let ie, X;
      var re = c(Y);
      {
        var ce = (Q) => {
          var I = oa(), q = c(I);
          Ye(q, { ariaLabel: "Select Row", get checked() {
            return t(C)[j(D())];
          }, set checked($) {
            t(C)[j(D())] = $;
          } }), i(I), a(Q, I);
        };
        U(re, (Q) => {
          e.select && r()[0] && Q(ce);
        });
      }
      var J = z(re);
      Se(J, 17, T, Te, (Q, I, q) => {
        var $ = ue(), de = ee($);
        {
          var we = (pe) => {
            var Me = da(), mt = c(Me);
            {
              var Ve = (Ue) => {
                const vt = le(() => t(I).href || "");
                xt(Ue, { get href() {
                  return t(vt);
                }, children: (ct, kt) => {
                  var me = na();
                  let qe;
                  var Ze = c(me, true);
                  i(me), L((et) => {
                    qe = Ce(me, 1, "linkText nowrap svelte-12u4ifk", null, qe, et), se(Ze, t(I).content);
                  }, [() => ({ muted: t(I).muted })]), a(ct, me);
                }, $$slots: { default: true } });
              }, st = (Ue, vt) => {
                {
                  var ct = (me) => {
                    const qe = le(() => t(I).href || "");
                    xt(me, { get href() {
                      return t(qe);
                    }, target: "_blank", children: (Ze, et) => {
                      var ke = la();
                      let He;
                      var tt = c(ke, true);
                      i(ke), L((Be) => {
                        He = Ce(ke, 1, "linkText nowrap svelte-12u4ifk", null, He, Be), se(tt, t(I).content);
                      }, [() => ({ muted: t(I).muted })]), a(Ze, ke);
                    }, $$slots: { default: true } });
                  }, kt = (me, qe) => {
                    {
                      var Ze = (ke) => {
                        Pe(ke, { invisible: true, onclick: () => Yr(t(I).content.toString()), children: (He, tt) => {
                          var Be = ia();
                          let Le;
                          var Oe = c(Be, true);
                          i(Be), L((We) => {
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
                            } }), i(Oe), L((Re) => We = Ce(Oe, 1, "checkIcon nowrap svelte-12u4ifk", null, We, Re), [() => ({ muted: t(I).muted })]), a(Le, Oe);
                          }, Be = (Le, Oe) => {
                            {
                              var We = (Re) => {
                                Pe(Re, { invisible: true, onclick: (ze) => {
                                  var _a2, _b;
                                  return (_b = (_a2 = t(I)).onClick) == null ? void 0 : _b.call(_a2, ze, j(D()));
                                }, children: (ze, ut) => {
                                  var Qe = va();
                                  let rt;
                                  var jt = c(Qe, true);
                                  i(Qe), L((Ut) => {
                                    rt = Ce(Qe, 1, "onclick nowrap svelte-12u4ifk", null, rt, Ut), se(jt, t(I).content);
                                  }, [() => ({ muted: t(I).muted })]), a(ze, Qe);
                                }, $$slots: { default: true } });
                              }, dt = (Re) => {
                                var ze = ca();
                                let ut;
                                var Qe = c(ze, true);
                                i(ze), L((rt) => {
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
                        ((_a2 = e.columns[q]) == null ? void 0 : _a2.showAs) === "copyToClip" ? ke(Ze) : ke(et, false);
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
                ((_a2 = e.columns[q]) == null ? void 0 : _a2.showAs) === "a" ? Ue(Ve) : Ue(st, false);
              });
            }
            i(Me), a(pe, Me);
          };
          U(de, (pe) => {
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
            return t(R)[D()];
          }, set close(de) {
            t(R)[D()] = de;
          }, button: (de) => {
            var we = ua(), pe = c(we);
            Gr(pe, {}), i(we), a(de, we);
          }, children: (de, we) => {
            var pe = ue(), Me = ee(pe);
            Ie(Me, () => e.options, T, () => t(R)[D()]), a(de, pe);
          }, $$slots: { button: true, default: true } }), i(I), a(Q, I);
        };
        U(ne, (Q) => {
          e.options && r()[r().length - 1] && Q(ge);
        });
      }
      i(Y), L((Q, I) => {
        ie = Ce(Y, 1, "svelte-12u4ifk", null, ie, Q), X = Ae(Y, "", X, { "grid-template-columns": I });
      }, [() => ({ highlight: t(g) === D() }), w]), a(h, Y);
    };
    var Dt = c($e);
    {
      var Bt = (h) => {
        var T = ue(), D = ee(T);
        {
          var Y = (ie) => {
            var X = ue(), re = ee(X);
            Se(re, 17, () => e.rows, Te, (ce, J, ne) => {
              v(ce, () => t(J), () => ne);
            }), a(ie, X);
          };
          U(D, (ie) => {
            t(C).length === e.rows.length && ie(Y);
          });
        }
        a(h, T);
      }, Ot = (h) => {
        var T = ue(), D = ee(T);
        Se(D, 17, () => t(E), Te, (Y, ie, X) => {
          v(Y, () => t(ie), () => X);
        }), a(h, T);
      };
      U(Dt, (h) => {
        t(F) ? h(Bt) : h(Ot, false);
      });
    }
    i($e), Ne($e, (h) => y(te, h), () => t(te));
  }
  var _t = z($e), bt = c(_t), wt = c(bt);
  const Rt = le(() => `-${u.length * 1.4 + 3}rem`);
  lt(wt, { ariaLabel: "Select Columns", get offsetTop() {
    return t(Rt);
  }, btnInvisible: true, button: (h) => {
    var T = ga(), D = c(T);
    Xt(D, {}), i(T), a(h, T);
  }, children: (h, T) => {
    var D = ma(), Y = c(D);
    {
      var ie = (J) => {
        var ne = _a(), ge = c(ne);
        Ye(ge, { ariaLabel: "Select Column: Select", get checked() {
          return r()[0];
        }, set checked(Q) {
          r(r()[0] = Q, true);
        }, children: (Q, I) => {
          nt();
          var q = at("Select");
          a(Q, q);
        }, $$slots: { default: true } }), i(ne), a(J, ne);
      };
      U(Y, (J) => {
        e.select && J(ie);
      });
    }
    var X = z(Y, 2);
    Se(X, 17, () => e.columns, Te, (J, ne, ge) => {
      var Q = ba(), I = c(Q);
      const q = le(() => `Select Column: ${t(ne).content}`);
      Ye(I, { get ariaLabel() {
        return t(q);
      }, get checked() {
        return r()[e.select ? ge + 1 : ge];
      }, set checked($) {
        r(r()[e.select ? ge + 1 : ge] = $, true);
      }, children: ($, de) => {
        nt();
        var we = at();
        L(() => se(we, t(ne).content)), a($, we);
      }, $$slots: { default: true } }), i(Q), a(J, Q);
    });
    var re = z(X, 2);
    {
      var ce = (J) => {
        var ne = wa(), ge = c(ne);
        Ye(ge, { ariaLabel: "Select Column: Options", get checked() {
          return r()[r().length - 1];
        }, set checked(Q) {
          r(r()[r().length - 1] = Q, true);
        }, children: (Q, I) => {
          nt();
          var q = at("Options");
          a(Q, q);
        }, $$slots: { default: true } }), i(ne), a(J, ne);
      };
      U(re, (J) => {
        e.options && J(ce);
      });
    }
    i(D), a(h, D);
  }, $$slots: { button: true, default: true } });
  var it = z(wt, 2), zt = c(it);
  Ie(zt, () => e.caption ?? ot), i(it);
  var Mt = z(it, 2);
  {
    var At = (v) => {
      var h = ka();
      a(v, h);
    }, Et = (v) => {
      Ar(v, { get items() {
        return e.rows;
      }, get compact() {
        return d();
      }, get itemsPaginated() {
        return t(E);
      }, set itemsPaginated(h) {
        y(E, h, true);
      }, get page() {
        return t(s);
      }, set page(h) {
        y(s, h, true);
      }, get pageSize() {
        return t(b);
      }, set pageSize(h) {
        y(b, h, true);
      } });
    };
    U(Mt, (v) => {
      t(F) ? v(At) : v(Et, false);
    });
  }
  i(bt), i(_t), i(K), L((v) => {
    f(K, "aria-colcount", u.length), f(K, "aria-rowcount", t(H)), je = Ae(K, "", je, { width: e.width, "max-width": e.maxWidth }), De = Ae(Ke, "", De, { "grid-template-columns": v });
  }, [w]), a(n, K), be();
}
Ee(["mousedown"]);
var pa = k("<p>no results</p>");
function Ca(n, e) {
  _e(e, true);
  let r = A(oe([])), d = A(oe([]));
  ve(() => {
    let s = [], b = [];
    if (e.rows.length > 0) {
      for (let B of e.rows[0].columns) s.push({ content: B.name, initialWidth: "12rem", orderType: o(B.value) });
      for (let B of e.rows) {
        let C = [];
        for (let N of B.columns) C.push({ content: x(N.value) });
        b.push(C);
      }
    }
    y(r, s, true), y(d, b, true);
  });
  function l(s) {
    return [...new Uint8Array(s)].map((b) => b.toString(16).padStart(2, "0")).join("");
  }
  function o(s) {
    return s.hasOwnProperty("Integer") || s.hasOwnProperty("Real") ? "number" : "string";
  }
  function x(s) {
    return s.hasOwnProperty("Integer") ? s.Integer : s.hasOwnProperty("Real") ? s.Real : s.hasOwnProperty("Text") ? s.Text : s.hasOwnProperty("Blob") ? `x'${l(s.Blob)}'` : "NULL";
  }
  var m = ue(), S = ee(m);
  {
    var u = (s) => {
      xa(s, { get columns() {
        return t(r);
      }, paginationPageSize: 100, get rows() {
        return t(d);
      }, set rows(b) {
        y(d, b, true);
      } });
    }, M = (s) => {
      var b = pa();
      a(s, b);
    };
    U(S, (s) => {
      t(r).length > 0 && t(d).length > 0 ? s(u) : s(M, false);
    });
  }
  a(n, m), be();
}
function Sa(n, e) {
  n.ctrlKey && n.code === "Enter" && e();
}
var Pa = k('<div role="textbox" tabindex="0" class="query svelte-1o8x9h5" contenteditable=""></div>'), La = k('<div class="err"> </div>'), Ta = k('<!> <!> <div id="query-results" class="svelte-1o8x9h5"><!></div>', 1);
function Ia(n, e) {
  _e(e, true);
  let r = _(e, "query", 7), d = A(oe([])), l = A(""), o = A(void 0), x = A(void 0), m = le(() => t(o) && t(x) ? `${t(o) - t(x)}px` : "100%");
  ve(() => {
    r().query.startsWith(yt) && (r().query = r().query.replace(`${yt}
`, ""), S());
  });
  async function S() {
    y(d, [], true), y(l, "");
    let R = [];
    for (let F of r().query.split(/\r?\n/)) F.startsWith("--") || R.push(F);
    let G = R.join(`
`), E = await Kt("/query", G);
    if (E.status === 200) y(d, await E.json(), true);
    else {
      let F = await E.json();
      y(l, Object.values(F)[0], true);
    }
  }
  async function u(R) {
    y(x, R, true);
  }
  var M = Ta(), s = ee(M);
  Gt(s, { resizeBottom: true, minHeightPx: 100, initialHeightPx: 300, onResizeBottom: u, children: (R, G) => {
    var E = Pa();
    E.__keydown = [Sa, S], or("innerText", E, () => r().query, (F) => r().query = F), a(R, E);
  }, $$slots: { default: true } });
  var b = z(s, 2);
  {
    var B = (R) => {
      var G = La(), E = c(G, true);
      i(G), L(() => se(E, t(l))), a(R, G);
    };
    U(b, (R) => {
      t(l) && R(B);
    });
  }
  var C = z(b, 2);
  let N;
  var O = c(C);
  Ca(O, { get rows() {
    return t(d);
  }, set rows(R) {
    y(d, R, true);
  } }), i(C), L(() => N = Ae(C, "", N, { height: t(m), "max-height": t(m) })), nr("innerHeight", (R) => y(o, R, true)), a(n, M), be();
}
Ee(["keydown"]);
var Da = ye('<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12"></path></svg>');
function Ba(n, e) {
  let r = _(e, "color", 8, "var(--col-err)"), d = _(e, "opacity", 8, 0.9), l = _(e, "width", 8, "1.5rem");
  var o = Da();
  f(o, "stroke-width", 2), L(() => {
    f(o, "width", l()), f(o, "color", r()), f(o, "opacity", d());
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
  Ie(M, () => e.children), i(u), Ne(u, (C) => l = C, () => l);
  var s = z(u, 2), b = c(s);
  b.__click = [pt, e, r], b.__keydown = [pt, e, r];
  var B = c(b);
  Ba(B, { color: "hsl(var(--error))", width: "1.2rem" }), i(b), i(s), i(S), L(() => {
    Ce(u, 1, Jt(t(o) ? "tab selected" : "tab"), "svelte-1ml8s23"), f(u, "contenteditable", t(o));
  }), ht("blur", u, x), a(n, S), be();
}
Ee(["click", "keydown"]);
var Ma = ye('<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15"></path></svg>');
function Aa(n, e) {
  let r = _(e, "opacity", 8, 0.9), d = _(e, "width", 8, "1.5rem");
  var l = Ma();
  f(l, "stroke-width", 2), L(() => {
    f(l, "width", d()), f(l, "opacity", r());
  }), a(n, l);
}
function Ct() {
  ae.push({ id: Ge(6), query: $t });
}
var Ea = k('<div id="tabs" class="svelte-ko98zn"><!> <div role="button" tabindex="0" title="Add New Tab" class="ctrl add-new svelte-ko98zn"><!></div></div> <!>', 1);
function ja(n, e) {
  _e(e, true);
  let r = A(oe(ae[0].id)), d = le(() => ae.filter((s) => s.id === t(r))[0]);
  ve(() => {
    ae.length > 0 ? y(r, ae[ae.length - 1].id, true) : y(r, "");
  });
  function l(s) {
    let B = ae.map((C) => C.id).indexOf(s);
    t(r) === s ? ae.length === 1 ? (ae.push(er), ae.shift(), y(r, ae[0].id, true)) : B === 0 ? (ae.shift(), y(r, ae[0].id, true)) : (ae.splice(B, 1), y(r, ae[B - 1].id, true)) : ae.splice(B, 1);
  }
  var o = Ea(), x = ee(o), m = c(x);
  Se(m, 17, () => ae, (s) => s.id, (s, b, B) => {
    za(s, { onClose: l, get tab() {
      return t(b).id;
    }, set tab(C) {
      t(b).id = C;
    }, get tabSelected() {
      return t(r);
    }, set tabSelected(C) {
      y(r, C, true);
    }, children: (C, N) => {
      nt();
      var O = at();
      L(() => se(O, t(b).id)), a(C, O);
    }, $$slots: { default: true } });
  });
  var S = z(m, 2);
  S.__click = [Ct], S.__keydown = [Ct];
  var u = c(S);
  Aa(u, {}), i(S), i(x);
  var M = z(x, 2);
  Ia(M, { get query() {
    return t(d);
  } }), a(n, o), be();
}
Ee(["click", "keydown"]);
var Ua = k('<meta property="description" content="Hiqlite Dashboard"/>');
function Va(n) {
  Wt((e) => {
    var r = Ua();
    Nt.title = "Hiqlite", a(e, r);
  }), ja(n, {});
}
export {
  Va as component
};
