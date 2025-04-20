import { l as Nt, r as Qt, w as Ft, t as k, a as l, c as ue, p as gt, o as ye, e as qe, s as se, b as ot, q as Vt } from "../chunks/B3zjZD7z.js";
import "../chunks/ByraeRS2.js";
import { C as Pt, aT as Zt, D as Yt, p as _e, k as d, l as s, t as L, g as t, a5 as le, j as be, a as ve, a3 as A, i as ee, s as M, N as x, W as oe, b as Ge, G as nt, aP as lt, aO as Gt } from "../chunks/DEAb5m-A.js";
import { s as De, a as f, j as Se, m as Xe, B as Pe, r as Lt, b as Ae, d as Kt, g as Ce, i as Te, o as Xt, p as Jt, I as $t, R as er, n as xt, q as tr, k as rr, Q as ae, u as ar, D as or } from "../chunks/BRdZZJSH.js";
import { p as b, s as nr, a as lr, i as j, b as Qe } from "../chunks/COt1BKSo.js";
import { s as ir } from "../chunks/X1x_5zTn.js";
function Tt(a, e, r) {
  if (a.multiple) return cr(a, e);
  for (var i of a.options) {
    var o = Ke(i);
    if (Zt(o, e)) {
      i.selected = true;
      return;
    }
  }
  (!r || e !== void 0) && (a.selectedIndex = -1);
}
function sr(a, e) {
  Pt(() => {
    var r = new MutationObserver(() => {
      var i = a.__value;
      Tt(a, i);
    });
    return r.observe(a, { childList: true, subtree: true, attributes: true, attributeFilter: ["value"] }), () => {
      r.disconnect();
    };
  });
}
function vr(a, e, r = e) {
  var i = true;
  Nt(a, "change", (o) => {
    var n = o ? "[selected]" : ":checked", y;
    if (a.multiple) y = [].map.call(a.querySelectorAll(n), Ke);
    else {
      var _ = a.querySelector(n) ?? a.querySelector("option:not([disabled])");
      y = _ && Ke(_);
    }
    r(y);
  }), Pt(() => {
    var o = e();
    if (Tt(a, o, i), i && o === void 0) {
      var n = a.querySelector(":checked");
      n !== null && (o = Ke(n), r(o));
    }
    a.__value = o, i = false;
  }), sr(a);
}
function cr(a, e) {
  for (var r of a.options) r.selected = ~e.indexOf(Ke(r));
}
function Ke(a) {
  return "__value" in a ? a.__value : a.value;
}
function dr(a, e, r, i = r) {
  e.addEventListener("input", () => {
    i(e[a]);
  }), Yt(() => {
    var o = r();
    if (e[a] !== o) if (o == null) {
      var n = e[a];
      i(n);
    } else e[a] = o + "";
  });
}
function ur(a, e) {
  Qt(window, ["resize"], () => Ft(() => e(window[a])));
}
const fr = () => {
  const a = ir;
  return { page: { subscribe: a.page.subscribe }, navigating: { subscribe: a.navigating.subscribe }, updated: a.updated };
}, hr = { subscribe(a) {
  return fr().page.subscribe(a);
} };
var gr = k('<span class="font-label"><a><!></a></span>');
function pt(a, e) {
  _e(e, true);
  const [r, i] = nr(), o = () => lr(hr, "$page", r);
  let n = b(e, "selectedStep", 3, false), y = b(e, "hideUnderline", 3, false), _ = le(() => {
    if (n()) return "step";
    if (o().route.id === e.href.split("?")[0]) return "page";
  });
  var C = gr(), u = d(C);
  let z;
  var v = d(u);
  De(v, () => e.children), s(u), s(C), L((w) => {
    f(u, "href", e.href), f(u, "target", e.target), f(u, "aria-current", t(_)), z = Se(u, 1, "svelte-a0xtvp", null, z, w);
  }, [() => ({ hideUnderline: y() })]), l(a, C), be(), i();
}
var _r = k('<!> <div class="popover svelte-1au8ouo" popover="auto"><div class="inner fade-in svelte-1au8ouo"><!></div></div>', 1);
function it(a, e) {
  _e(e, true);
  let r = b(e, "ref", 15), i = b(e, "roleButton", 3, "button"), o = b(e, "offsetLeft", 3, "0px"), n = b(e, "offsetTop", 3, "0px"), y = b(e, "close", 15);
  const _ = Xe(8), C = Xe(8);
  let u = A(void 0), z = A(false);
  ve(() => {
    y(w);
  });
  function v() {
    if (r() && t(u)) if (e.absolute) t(u).style.top = n(), t(u).style.left = o();
    else {
      let H = r().getBoundingClientRect();
      t(u).style.top = `calc(${H.bottom + window.scrollY}px + ${n()})`, t(u).style.left = `calc(${H.left + window.scrollX}px + ${o()})`;
    }
    else console.error("button and popover ref missing");
  }
  function w() {
    var _a2;
    (_a2 = t(u)) == null ? void 0 : _a2.hidePopover();
  }
  function O(H) {
    var _a2;
    let Z = H.newState;
    x(z, Z === "open"), (_a2 = e.onToggle) == null ? void 0 : _a2.call(e, Z);
  }
  var S = _r(), Q = ee(S);
  Pe(Q, { get role() {
    return i();
  }, id: _, ariaControls: C, popovertarget: C, onclick: v, get invisible() {
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
    De(p, () => e.button), l(H, te);
  }, $$slots: { default: true } });
  var B = M(Q, 2);
  f(B, "id", C), f(B, "aria-labelledby", _);
  var R = d(B), K = d(R);
  {
    var q = (H) => {
      var Z = ue(), te = ee(Z);
      {
        var p = (g) => {
          var P = ue(), m = ee(P);
          De(m, () => e.children), l(g, P);
        };
        j(te, (g) => {
          t(z) && g(p);
        });
      }
      l(H, Z);
    }, F = (H) => {
      var Z = ue(), te = ee(Z);
      De(te, () => e.children), l(H, Z);
    };
    j(K, (H) => {
      e.lazy ? H(q) : H(F, false);
    });
  }
  s(R), s(B), Qe(B, (H) => x(u, H), () => t(u)), L(() => f(B, "aria-label", e.ariaLabel)), gt("toggle", B, O), l(a, S), be();
}
var br = ye('<svg fill="none" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M12 9.75L14.25 12m0 0l2.25 2.25M14.25 12l2.25-2.25M14.25 12L12 14.25m-2.58 4.92l-6.375-6.375a1.125 1.125 0 010-1.59L9.42 4.83c.211-.211.498-.33.796-.33H19.5a2.25 2.25 0 012.25 2.25v10.5a2.25 2.25 0 01-2.25 2.25h-9.284c-.298 0-.585-.119-.796-.33z"></path></svg>');
function wr(a, e) {
  let r = b(e, "color", 8, "currentColor"), i = b(e, "opacity", 8, 0.9), o = b(e, "width", 8, "1.5rem");
  var n = br();
  f(n, "stroke-width", 2), L(() => {
    f(n, "stroke", r()), f(n, "width", o()), f(n, "opacity", i());
  }), l(a, n);
}
var mr = ye('<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M21 21l-5.197-5.197m0 0A7.5 7.5 0 105.196 5.196a7.5 7.5 0 0010.607 10.607z"></path></svg>');
function kr(a, e) {
  let r = b(e, "opacity", 8, 0.9), i = b(e, "width", 8, "1.5rem");
  var o = mr();
  f(o, "stroke-width", 2), L(() => {
    f(o, "width", i()), f(o, "opacity", r());
  }), l(a, o);
}
function yr(a, e, r, i) {
  var _a2, _b, _c;
  switch (a.code) {
    case "Enter":
      e();
      break;
    case "Tab":
      (_a2 = r.onTab) == null ? void 0 : _a2.call(r, i());
      break;
    case "ArrowUp":
      (_b = r.onUp) == null ? void 0 : _b.call(r, i());
      break;
    case "ArrowDown":
      (_c = r.onDown) == null ? void 0 : _c.call(r, i());
      break;
  }
}
var xr = k('<div class="options svelte-13lxusw"><!></div>'), pr = k("<option></option>"), Sr = k('<datalist class="absolute svelte-13lxusw"></datalist>'), Cr = k('<div class="magnify svelte-13lxusw"><!></div>'), Pr = k('<div class="btnSearch svelte-13lxusw"><!></div>'), Lr = k('<search class="flex container svelte-13lxusw"><!> <input type="search" autocomplete="off" aria-label="Search" placeholder="Search" class="svelte-13lxusw"> <!> <div class="relative"><div class="absolute btnDelete svelte-13lxusw"><!></div></div> <!></search>');
function Tr(a, e) {
  _e(e, true);
  let r = b(e, "value", 15, ""), i = b(e, "option", 15), o = b(e, "focus", 15), n = b(e, "width", 3, "100%");
  const y = Xe(8), _ = Xe(8);
  let C = A(void 0), u = le(() => e.datalist && e.datalist.length > 0 ? _ : void 0);
  ve(() => {
    o(v);
  });
  function z() {
    var _a2;
    (_a2 = e.onSearch) == null ? void 0 : _a2.call(e, r());
  }
  function v() {
    var _a2;
    (_a2 = t(C)) == null ? void 0 : _a2.focus();
  }
  var w = Lr();
  let O;
  var S = d(w);
  {
    var Q = (p) => {
      var g = xr(), P = d(g);
      Dt(P, { ariaLabel: "Search Options", get options() {
        return e.options;
      }, borderless: true, get value() {
        return i();
      }, set value(m) {
        i(m);
      } }), s(g), l(p, g);
    };
    j(S, (p) => {
      e.options && p(Q);
    });
  }
  var B = M(S, 2);
  Lt(B), f(B, "id", y), B.__keydown = [yr, z, e, r], Qe(B, (p) => x(C, p), () => t(C));
  var R = M(B, 2);
  {
    var K = (p) => {
      var g = Sr();
      f(g, "id", _), Ce(g, 21, () => e.datalist, Te, (P, m, W, E) => {
        var V = pr(), fe = {};
        L(() => {
          fe !== (fe = t(m)) && (V.value = (V.__value = t(m)) ?? "");
        }), l(P, V);
      }), s(g), l(p, g);
    };
    j(R, (p) => {
      e.datalist && p(K);
    });
  }
  var q = M(R, 2), F = d(q), H = d(F);
  Pe(H, { ariaLabel: "Delete Search Input", invisible: true, onclick: () => r(""), children: (p, g) => {
    wr(p, { color: "hsl(var(--bg-high))", width: 24 });
  }, $$slots: { default: true } }), s(F), s(q);
  var Z = M(q, 2);
  {
    var te = (p) => {
      var g = Pr(), P = d(g);
      Pe(P, { ariaLabel: "Search", invisible: true, onclick: z, children: (m, W) => {
        var E = Cr(), V = d(E);
        kr(V, {}), s(E), l(m, E);
      }, $$slots: { default: true } }), s(g), l(p, g);
    };
    j(Z, (p) => {
      e.onSearch && p(te);
    });
  }
  s(w), L(() => {
    O = Ae(w, "", O, { border: e.borderless ? void 0 : "1px solid hsl(var(--bg-high))", width: n() }), f(B, "list", t(u));
  }), gt("focus", B, () => {
    var _a2;
    return (_a2 = e.onFocus) == null ? void 0 : _a2.call(e);
  }), Kt(B, r), l(a, w), be();
}
qe(["keydown"]);
var Dr = ye('<svg fill="none" viewBox="0 0 24 24" color="currentColor" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M8.25 4.5l7.5 7.5-7.5 7.5"></path></svg>');
function ht(a, e) {
  let r = b(e, "opacity", 8, 0.9), i = b(e, "width", 8, "1.5rem");
  var o = Dr();
  f(o, "stroke-width", 2), L(() => {
    f(o, "width", i()), f(o, "opacity", r());
  }), l(a, o);
}
function Ir(a, e, r, i, o) {
  let n = a.code;
  n === "ArrowDown" ? (a.preventDefault(), e() && x(r, t(r) + 1)) : n === "ArrowUp" ? (a.preventDefault(), e() && x(r, t(r) - 1)) : n === "Enter" && t(r) > -1 ? i(t(o)[t(r)]) : n === "Enter" && t(r) === -1 && t(o).length === 1 && i(t(o)[0]);
}
var Or = k('<div class="btn svelte-1j5gmms"> <div class="chevron svelte-1j5gmms"><!></div></div>'), Br = k('<div class="optPopover svelte-1j5gmms"> </div>'), Rr = k('<div role="listbox" tabindex="0" class="popover svelte-1j5gmms"><!> <div class="popoverOptions svelte-1j5gmms"></div></div>'), Mr = k('<option class="opt svelte-1j5gmms"> </option>'), zr = k("<select></select>");
function Dt(a, e) {
  _e(e, true);
  let r = b(e, "ref", 15), i = b(e, "options", 19, () => []), o = b(e, "value", 15), n = b(e, "asPopover", 3, true), y = b(e, "borderless", 3, false), _ = b(e, "withSearch", 3, false), C = b(e, "fallbackOptions", 3, false), u = A(void 0), z = A(oe(C() ? false : n())), v = A(void 0), w = A(oe(_() ? -1 : 0)), O = A(void 0), S = A(""), Q = le(() => {
    if (!_()) return i();
    if (typeof o() == "string") return i().filter((g) => g.toLowerCase().includes(t(S).toLowerCase()));
    let p = Number.parseInt(t(S)) || o();
    return i().filter((g) => g === p);
  });
  ve(() => {
    t(z) !== n() && x(z, n());
  }), ve(() => {
    var _a2, _b;
    if (t(w) === -1 && ((_a2 = t(u)) == null ? void 0 : _a2.scrollTo({ top: 0, behavior: "smooth" })), _()) {
      if (t(w) < 0 || t(w) > t(Q).length - 1) {
        x(w, -1), (_b = t(O)) == null ? void 0 : _b();
        return;
      }
    } else t(w) < 0 ? x(w, t(Q).length - 1) : t(w) > t(Q).length - 1 && x(w, 0), B();
  });
  function B() {
    if (t(u)) {
      let p = t(u).getElementsByTagName("button")[t(w)];
      p.scrollIntoView({ behavior: "smooth", block: "center" }), p.focus();
    } else console.error("refOptions is undefined");
  }
  function R(p) {
    var _a2;
    p === "open" && (_() ? (x(w, -1), (_a2 = t(O)) == null ? void 0 : _a2()) : (x(w, i().findIndex((g) => g === o()) || 0, true), B()));
  }
  function K() {
    return t(Q).length > 0 ? true : (x(w, -1), false);
  }
  function q(p) {
    o(p), x(S, ""), setTimeout(() => {
      var _a2;
      (_a2 = t(v)) == null ? void 0 : _a2();
    }, 20);
  }
  var F = ue(), H = ee(F);
  {
    var Z = (p) => {
      it(p, { get ariaLabel() {
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
        return t(v);
      }, set close(P) {
        x(v, P, true);
      }, button: (P) => {
        var m = Or(), W = d(m), E = M(W), V = d(E);
        ht(V, { width: 14 }), s(E), s(m), L(() => {
          f(m, "data-border", !y()), se(W, `${o() ?? ""} `);
        }), l(P, m);
      }, children: (P, m) => {
        var W = Rr();
        W.__keydown = [Ir, K, w, q, Q];
        let E;
        var V = d(W);
        {
          var fe = (he) => {
            Tr(he, { onFocus: () => x(w, -1), get value() {
              return t(S);
            }, set value(X) {
              x(S, X, true);
            }, get focus() {
              return t(O);
            }, set focus(X) {
              x(O, X, true);
            } });
          };
          j(V, (he) => {
            _() && he(fe);
          });
        }
        var xe = M(V, 2);
        Ce(xe, 21, () => t(Q), Te, (he, X, Ee) => {
          Pe(he, { invisible: true, invisibleOutline: true, onclick: () => q(t(X)), children: (Fe, Je) => {
            var Ie = Br(), $e = d(Ie, true);
            s(Ie), L(() => {
              f(Ie, "aria-selected", o() === t(X)), f(Ie, "data-focus", t(w) === Ee), se($e, t(X));
            }), l(Fe, Ie);
          }, $$slots: { default: true } });
        }), s(xe), Qe(xe, (he) => x(u, he), () => t(u)), s(W), L(() => E = Ae(W, "", E, { "max-height": e.maxHeight })), l(P, W);
      }, $$slots: { button: true, default: true } });
    }, te = (p) => {
      var g = zr();
      let P;
      Ce(g, 21, () => t(Q), Te, (m, W) => {
        var E = Mr(), V = {}, fe = d(E, true);
        s(E), L(() => {
          V !== (V = t(W)) && (E.value = (E.__value = t(W)) ?? ""), Xt(E, o() === t(W)), se(fe, t(W));
        }), l(m, E);
      }), s(g), L((m) => {
        f(g, "name", e.name), f(g, "aria-label", e.ariaLabel), P = Se(g, 1, "svelte-1j5gmms", null, P, m);
      }, [() => ({ borderless: y() })]), vr(g, o), l(p, g);
    };
    j(H, (p) => {
      t(z) ? p(Z) : p(te, false);
    });
  }
  l(a, F), be();
}
qe(["keydown"]);
var Ar = k('<div class="link noselect svelte-1bye1t3"> </div>'), qr = k('<li class="svelte-1bye1t3"><!></li>'), Er = k('<nav aria-label="Pagination" class="svelte-1bye1t3"><ul class="svelte-1bye1t3"></ul></nav>'), jr = k('<div class="flex gap-10 svelte-1bye1t3"><div class="flex gap-05 chunkSize noselect svelte-1bye1t3"><div class="svelte-1bye1t3">Entries</div> <div class="svelte-1bye1t3"><!></div></div> <div class="font-label total svelte-1bye1t3"> </div></div>'), Ur = k('<div class="iconLeft svelte-1bye1t3" aria-label="Go to previous page"><!></div>'), Hr = k('<div class="iconRight svelte-1bye1t3" aria-label="Go to next page"><!></div>'), Wr = k('<div class="container svelte-1bye1t3"><!> <!> <!> <!></div>');
function Nr(a, e) {
  _e(e, true);
  const r = (g) => {
    var P = Er(), m = d(P);
    Ce(m, 21, () => t(w), Te, (W, E) => {
      var V = qr(), fe = d(V);
      Pe(fe, { invisible: true, onclick: () => Q(t(E)), onLeft: O, onRight: S, children: (xe, he) => {
        var X = Ar(), Ee = d(X, true);
        s(X), L(() => se(Ee, t(E))), l(xe, X);
      }, $$slots: { default: true } }), s(V), L(() => {
        f(V, "aria-label", `go to page number: ${t(E)}`), f(V, "aria-current", y() === t(E) ? "step" : void 0);
      }), l(W, V);
    }), s(m), s(P), l(g, P);
  }, i = (g) => {
    var P = jr(), m = d(P), W = M(d(m), 2), E = d(W);
    Dt(E, { ariaLabel: "Page Count", options: o, offsetTop: "-17rem", borderless: true, get value() {
      return _();
    }, set value(xe) {
      _(xe);
    } }), s(W), s(m);
    var V = M(m, 2), fe = d(V);
    s(V), s(P), L(() => se(fe, `Total: ${e.items.length ?? ""}`)), l(g, P);
  }, o = [5, 7, 10, 15, 20, 30, 50, 100];
  let n = b(e, "itemsPaginated", 15), y = b(e, "page", 15, 1), _ = b(e, "pageSize", 31, () => oe(o[0])), C = b(e, "compact", 3, false);
  const u = 16;
  let z = Ge(() => _()), v = A(oe([])), w = A(oe([]));
  ve(() => {
    _() !== z && (z = Ge(() => _()), y(1));
  }), ve(() => {
    let g = [];
    for (let P = 0; P < e.items.length; P += _()) {
      const m = e.items.slice(P, P + _());
      g.push(m);
    }
    x(v, g, true), n(g[y() - 1]);
  }), ve(() => {
    B();
  });
  function O() {
    y() > 1 && Q(y() - 1);
  }
  function S() {
    y() < t(v).length && Q(y() + 1);
  }
  function Q(g) {
    y(g), B();
  }
  function B() {
    let g = [], P = Math.floor(_() / 2);
    if (t(v).length <= _()) for (let m = 1; m <= t(v).length; m++) g.push(m);
    else if (y() <= P) for (let m = 1; m <= _(); m++) g.push(m);
    else if (y() > t(v).length - P - 1) for (let m = t(v).length - _(); m <= t(v).length - 1; m++) g.push(m + 1);
    else for (let m = y() - P; m < y() - P + _(); m++) g.push(m);
    x(w, g, true);
  }
  var R = Wr(), K = d(R);
  const q = le(() => y() === 1);
  Pe(K, { onclick: O, invisible: true, get isDisabled() {
    return t(q);
  }, children: (g, P) => {
    var m = Ur(), W = d(m);
    ht(W, { width: u }), s(m), L(() => f(m, "data-disabled", y() === 1)), l(g, m);
  }, $$slots: { default: true } });
  var F = M(K, 2);
  r(F);
  var H = M(F, 2);
  const Z = le(() => y() === t(v).length);
  Pe(H, { onclick: S, invisible: true, get isDisabled() {
    return t(Z);
  }, children: (g, P) => {
    var m = Hr(), W = d(m);
    ht(W, { width: u }), s(m), L(() => f(m, "data-disabled", y() === t(v).length)), l(g, m);
  }, $$slots: { default: true } });
  var te = M(H, 2);
  {
    var p = (g) => {
      i(g);
    };
    j(te, (g) => {
      C() || g(p);
    });
  }
  s(R), l(a, R), be();
}
function Qr(a, e) {
  console.log(a.code), a.code === "Enter" && e();
}
var Fr = k('<label class="font-label noselect svelte-1supmpl"><input type="checkbox" class="svelte-1supmpl"> <span class="svelte-1supmpl"><!></span></label>');
function Ye(a, e) {
  _e(e, true);
  let r = b(e, "checked", 15, false), i = b(e, "ariaLabel", 3, ""), o = b(e, "borderColor", 3, "hsl(var(--bg-high))");
  function n() {
    r(!r());
  }
  var y = Fr(), _ = d(y);
  Lt(_), _.__click = n, _.__keydown = [Qr, n];
  let C;
  var u = M(_, 2), z = d(u);
  De(z, () => e.children ?? nt), s(u), s(y), L(() => {
    f(_, "name", e.name), _.disabled = e.disabled, f(_, "aria-disabled", e.disabled), f(_, "aria-checked", r()), f(_, "aria-label", i()), C = Ae(_, "", C, { "border-color": o() });
  }), Jt(_, r), l(a, y), be();
}
qe(["click", "keydown"]);
var Vr = ye('<svg fill="none" viewBox="0 0 24 24" stroke="currentColor" color="hsl(var(--action))"><path stroke-linecap="round" stroke-linejoin="round" d="M4.5 12.75l6 6 9-13.5"></path></svg>'), Zr = ye('<svg fill="none" viewBox="0 0 24 24" stroke="currentColor" color="hsl(var(--error))"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12"></path></svg>');
function Yr(a, e) {
  _e(e, true);
  const r = 20, i = 0.9;
  var o = ue(), n = ee(o);
  {
    var y = (C) => {
      var u = Vr();
      f(u, "stroke-width", 2), f(u, "width", r), f(u, "opacity", i), l(C, u);
    }, _ = (C) => {
      var u = Zr();
      f(u, "stroke-width", 2), f(u, "width", r), f(u, "opacity", i), l(C, u);
    };
    j(n, (C) => {
      e.checked ? C(y) : C(_, false);
    });
  }
  l(a, o), be();
}
var Gr = ye('<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m19.5 8.25-7.5 7.5-7.5-7.5"></path></svg>');
function Kr(a, e) {
  let r = b(e, "color", 8, "currentColor"), i = b(e, "opacity", 8, 0.9), o = b(e, "width", 8, "1.5rem");
  var n = Gr();
  f(n, "stroke-width", 2), L(() => {
    f(n, "width", o()), f(n, "color", r()), f(n, "opacity", i());
  }), l(a, n);
}
var Xr = ye('<svg fill="none" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M3 7.5 7.5 3m0 0L12 7.5M7.5 3v13.5m13.5 0L16.5 21m0 0L12 16.5m4.5 4.5V7.5"></path></svg>');
function Jr(a, e) {
  let r = b(e, "color", 8, "currentColor"), i = b(e, "opacity", 8, 0.9), o = b(e, "width", 8, "1.5rem");
  var n = Xr();
  f(n, "stroke-width", 2), L(() => {
    f(n, "stroke", r()), f(n, "width", o()), f(n, "opacity", i());
  }), l(a, n);
}
var $r = ye(`<svg fill="none" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213
            1.281c.063.374.313.686.645.87.074.04.147.083.22.127.325.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 0
            1 1.37.49l1.296 2.247a1.125 1.125 0 0 1-.26 1.431l-1.003.827c-.293.241-.438.613-.43.992a7.723 7.723 0 0
            1 0 .255c-.008.378.137.75.43.991l1.004.827c.424.35.534.955.26 1.43l-1.298 2.247a1.125 1.125 0 0
            1-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.47 6.47 0 0
            1-.22.128c-.331.183-.581.495-.644.869l-.213 1.281c-.09.543-.56.94-1.11.94h-2.594c-.55 0-1.019-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52
            0 0 1-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 0 1-1.369-.49l-1.297-2.247a1.125
            1.125 0 0 1 .26-1.431l1.004-.827c.292-.24.437-.613.43-.991a6.932 6.932 0 0 1 0-.255c.007-.38-.138-.751-.43-.992l-1.004-.827a1.125 1.125
            0 0 1-.26-1.43l1.297-2.247a1.125 1.125 0 0 1 1.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.086.22-.128.332-.183.582-.495.644-.869l.214-1.28Z"></path><path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z"></path></svg>`);
function ea(a, e) {
  let r = b(e, "color", 8, "currentColor"), i = b(e, "opacity", 8, 0.9), o = b(e, "width", 8, "1.5rem");
  var n = $r();
  f(n, "stroke-width", 2), L(() => {
    f(n, "stroke", r()), f(n, "width", o()), f(n, "opacity", i());
  }), l(a, n);
}
async function ta(a) {
  var _a2;
  await ((_a2 = navigator == null ? void 0 : navigator.clipboard) == null ? void 0 : _a2.writeText(a));
}
var ra = ye(`<svg fill="none" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M6.75 12a.75.75 0 1 1-1.5 0 .75.75 0 0 1 1.5 0ZM12.75 12a.75.75 0 1 1-1.5 0 .75.75 0 0 1 1.5
            0ZM18.75 12a.75.75 0 1 1-1.5 0 .75.75 0 0 1 1.5 0Z"></path></svg>`);
function aa(a, e) {
  let r = b(e, "color", 8, "currentColor"), i = b(e, "opacity", 8, 0.9), o = b(e, "width", 8, "1.5rem");
  var n = ra();
  f(n, "stroke-width", 2), L(() => {
    f(n, "stroke", r()), f(n, "width", o()), f(n, "opacity", i());
  }), l(a, n);
}
var oa = k('<span class="btnSelect svelte-12u4ifk"><!></span>'), na = k('<th class="headerCheckbox svelte-12u4ifk"><!> <!></th>'), la = k('<span class="iconOrder svelte-12u4ifk"><!></span>'), ia = k('<span class="orderText svelte-12u4ifk"> </span> <!>', 1), sa = k('<span class="rawText svelte-12u4ifk"> </span>'), va = k('<th class="svelte-12u4ifk"><span class="flex-1 label svelte-12u4ifk"><!></span> <span class="relative"><span role="none" class="absolute sizeable svelte-12u4ifk"></span></span></th>'), ca = k('<th class="headerOptions svelte-12u4ifk"><!></th>'), da = k('<td class="checkbox svelte-12u4ifk"><!></td>'), ua = k("<span> </span>"), fa = k("<span> </span>"), ha = k("<span> </span>"), ga = k("<span><!></span>"), _a = k("<span> </span>"), ba = k("<span> </span>"), wa = k('<td class="svelte-12u4ifk"><!></td>'), ma = k('<span class="btnOptions svelte-12u4ifk"><!></span>'), ka = k('<td class="options svelte-12u4ifk"><!></td>'), ya = k("<tr><!><!><!></tr>"), xa = k('<div class="eye svelte-12u4ifk"><!></div>'), pa = k('<div class="columnSelect svelte-12u4ifk"><!></div>'), Sa = k('<div class="columnSelect svelte-12u4ifk"><!></div>'), Ca = k('<div class="columnSelect svelte-12u4ifk"><!></div>'), Pa = k('<div class="columnsSelect svelte-12u4ifk"><!> <!> <!></div>'), La = k("<div></div>"), Ta = k('<table class="svelte-12u4ifk"><thead class="svelte-12u4ifk"><tr class="svelte-12u4ifk"><!><!><!></tr></thead><tbody class="svelte-12u4ifk"><!></tbody><caption class="flex space-between svelte-12u4ifk"><div class="flex"><!> <div class="caption svelte-12u4ifk"><!></div> <!></div></caption></table>');
function Da(a, e) {
  _e(e, true);
  let r = b(e, "showColumns", 31, () => oe(Array(e.columns.length).fill(true))), i = b(e, "paginationCompact", 3, false), o = b(e, "paginationPageSize", 3, 15), n = b(e, "selectInitHide", 3, false), y = b(e, "minWidthColPx", 3, 50);
  const _ = "3rem", C = "2rem";
  let u = oe(P()), z = A(oe(Ge(() => u))), v = A(1), w = A(oe(Ge(() => o()))), O = A(false), S = A(oe(Array(e.rows.length).fill(false))), Q = le(() => t(S).find((c) => c === true)), B = A(void 0), R = A(oe([])), K = A("up"), q = A(oe([])), F = le(() => e.paginationDisabled !== void 0 ? e.paginationDisabled : e.rows.length < o()), H = le(() => e.paginationDisabled || t(F) ? e.rows.length : t(q) && t(q).length ? t(q).length : (t(v) != 1 && x(v, 1), 0)), Z = oe(Array(Ge(() => u.length)).fill(void 0)), te = A(void 0), p = 0, g = A(void 0);
  ve(() => {
    setTimeout(() => {
      for (let c = 1; c < u.length; c++) if (u[c] === "auto") {
        p = e.select ? c : c - 1;
        let h = Z[c];
        h && he(h.getBoundingClientRect().width);
      }
    }, 150);
  }), ve(() => {
    let c = Array(e.rows.length).fill(false);
    if (t(O)) {
      let h;
      t(v) === 1 ? h = 0 : h = (t(v) - 1) * t(w);
      let T = Math.min(t(v) * t(w), e.rows.length);
      for (let I = h; I < T; I++) c[I] = true;
    }
    x(S, c, true);
  }), ve(() => {
    let c = e.paginationDisabled || t(F) ? e.rows.length : t(q).length || 0;
    x(R, Array(c).fill(() => console.error("un-initialized popover close option")), true);
  }), ve(() => {
    let c = [];
    for (let h = 0; h < u.length; h++) r()[h] && c.push(u[h]);
    x(z, c, true);
  }), ve(() => {
    if (e.highlight !== void 0 && t(te)) {
      let c = e.highlight;
      !t(F) && t(v) > 1 && (c = e.highlight - (t(v) - 1) * t(w)), x(g, c, true), setTimeout(() => {
        var _a2, _b;
        (_b = (_a2 = t(te)) == null ? void 0 : _a2.getElementsByClassName("highlight")[0]) == null ? void 0 : _b.scrollIntoView({ behavior: "smooth", block: "center" });
      }, 250);
    } else x(g, void 0);
  });
  function P() {
    let c = e.columns.map((T) => T.initialWidth), h = [...r()];
    return e.select && (c = [_, ...c], h = [!n(), ...h]), e.options && (c = [...c, C], h = [...h, true]), r(h), c;
  }
  function m() {
    return t(z).join(" ");
  }
  function W(c, h) {
    x(S, Array(e.rows.length).fill(false), true);
    let T = 1;
    t(K) === "up" ? (T = -1, x(K, "down")) : x(K, "up"), h === "string" ? e.rows.sort((I, Y) => I[c].content.localeCompare(Y[c].content) * T) : h === "number" && e.rows.sort((I, Y) => (I[c].content - Y[c].content) * T);
  }
  function E(c) {
    return !t(F) && t(v) > 1 ? (t(v) - 1) * t(w) + c : c;
  }
  function V(c) {
    p = c;
    let h = Z[c];
    h ? (he(h.getBoundingClientRect().width), window.addEventListener("mousemove", xe), window.addEventListener("mouseup", fe, { once: true })) : console.error("invalid ref from refCols in onMouseDown");
  }
  function fe() {
    window.removeEventListener("mousemove", xe);
  }
  function xe(c) {
    let h = Z[p];
    if (h) {
      let T = h.getBoundingClientRect().left, I = window.scrollX + c.x - T;
      he(I);
    } else console.error("invalid ref from refCols in onMove");
  }
  function he(c) {
    c = Math.ceil(c), c < y() && (c = y()), u[e.select ? p + 1 : p] = `${c}px`;
  }
  var X = Ta();
  let Ee;
  var Fe = d(X), Je = d(Fe);
  let Ie;
  var $e = d(Je);
  {
    var It = (c) => {
      var h = na(), T = d(h);
      Ye(T, { ariaLabel: "Select All", borderColor: "hsla(var(--text), .4)", get checked() {
        return t(O);
      }, set checked(ie) {
        x(O, ie, true);
      } });
      var I = M(T, 2);
      const Y = le(() => !t(Q));
      it(I, { ariaLabel: "Selected Options", get btnDisabled() {
        return t(Y);
      }, btnInvisible: true, get close() {
        return t(B);
      }, set close(G) {
        x(B, G, true);
      }, button: (G) => {
        var re = oa(), ce = d(re);
        Kr(ce, { width: "1rem" }), s(re), L(() => f(re, "data-disabled", !t(Q))), l(G, re);
      }, children: (G, re) => {
        var ce = ue(), J = ee(ce);
        De(J, () => e.select, () => t(S), () => t(B)), l(G, ce);
      }, $$slots: { button: true, default: true } }), s(h), l(c, h);
    };
    j($e, (c) => {
      e.select && r()[0] && c(It);
    });
  }
  var _t = M($e);
  Ce(_t, 17, () => e.columns, Te, (c, h, T) => {
    var I = ue(), Y = ee(I);
    {
      var ie = (G) => {
        var re = va(), ce = d(re), J = d(ce);
        {
          var ne = (U) => {
            var $ = ia(), de = ee($), we = d(de, true);
            s(de);
            var pe = M(de, 2);
            Pe(pe, { invisible: true, onclick: () => W(T, t(h).orderType), children: (ze, kt) => {
              var Ve = la(), vt = d(Ve);
              Jr(vt, { width: "1rem" }), s(Ve), l(ze, Ve);
            }, $$slots: { default: true } }), L(() => se(we, t(h).content)), l(U, $);
          }, ge = (U) => {
            var $ = sa(), de = d($, true);
            s($), L(() => se(de, t(h).content)), l(U, $);
          };
          j(J, (U) => {
            t(h).orderType ? U(ne) : U(ge, false);
          });
        }
        s(ce);
        var N = M(ce, 2), D = d(N);
        D.__mousedown = () => V(T), s(N), s(re), Qe(re, (U, $) => Z[$] = U, (U) => Z == null ? void 0 : Z[U], () => [T]), l(G, re);
      };
      j(Y, (G) => {
        r()[e.select ? T + 1 : T] && G(ie);
      });
    }
    l(c, I);
  });
  var Ot = M(_t);
  {
    var Bt = (c) => {
      var h = ca(), T = d(h);
      ea(T, { width: "1.2rem" }), s(h), l(c, h);
    };
    j(Ot, (c) => {
      e.options && r()[r().length - 1] && c(Bt);
    });
  }
  s(Je), s(Fe);
  var et = M(Fe);
  {
    const c = (h, T = nt, I = nt) => {
      var Y = ya();
      let ie, G;
      var re = d(Y);
      {
        var ce = (N) => {
          var D = da(), U = d(D);
          Ye(U, { ariaLabel: "Select Row", get checked() {
            return t(S)[E(I())];
          }, set checked($) {
            t(S)[E(I())] = $;
          } }), s(D), l(N, D);
        };
        j(re, (N) => {
          e.select && r()[0] && N(ce);
        });
      }
      var J = M(re);
      Ce(J, 17, T, Te, (N, D, U) => {
        var $ = ue(), de = ee($);
        {
          var we = (pe) => {
            var ze = wa(), kt = d(ze);
            {
              var Ve = (je) => {
                const ct = le(() => t(D).href || "");
                pt(je, { get href() {
                  return t(ct);
                }, children: (dt, yt) => {
                  var me = ua();
                  let Ue;
                  var Ze = d(me, true);
                  s(me), L((tt) => {
                    Ue = Se(me, 1, "linkText nowrap svelte-12u4ifk", null, Ue, tt), se(Ze, t(D).content);
                  }, [() => ({ muted: t(D).muted })]), l(dt, me);
                }, $$slots: { default: true } });
              }, vt = (je, ct) => {
                {
                  var dt = (me) => {
                    const Ue = le(() => t(D).href || "");
                    pt(me, { get href() {
                      return t(Ue);
                    }, target: "_blank", children: (Ze, tt) => {
                      var ke = fa();
                      let He;
                      var rt = d(ke, true);
                      s(ke), L((Oe) => {
                        He = Se(ke, 1, "linkText nowrap svelte-12u4ifk", null, He, Oe), se(rt, t(D).content);
                      }, [() => ({ muted: t(D).muted })]), l(Ze, ke);
                    }, $$slots: { default: true } });
                  }, yt = (me, Ue) => {
                    {
                      var Ze = (ke) => {
                        Pe(ke, { invisible: true, onclick: () => ta(t(D).content.toString()), children: (He, rt) => {
                          var Oe = ha();
                          let Le;
                          var Be = d(Oe, true);
                          s(Oe), L((We) => {
                            Le = Se(Oe, 1, "copyToClip nowrap svelte-12u4ifk", null, Le, We), se(Be, t(D).content);
                          }, [() => ({ muted: t(D).muted })]), l(He, Oe);
                        }, $$slots: { default: true } });
                      }, tt = (ke, He) => {
                        {
                          var rt = (Le) => {
                            var Be = ga();
                            let We;
                            var ut = d(Be);
                            Yr(ut, { get checked() {
                              return t(D).content;
                            } }), s(Be), L((Re) => We = Se(Be, 1, "checkIcon nowrap svelte-12u4ifk", null, We, Re), [() => ({ muted: t(D).muted })]), l(Le, Be);
                          }, Oe = (Le, Be) => {
                            {
                              var We = (Re) => {
                                Pe(Re, { invisible: true, onclick: (Me) => {
                                  var _a2, _b;
                                  return (_b = (_a2 = t(D)).onClick) == null ? void 0 : _b.call(_a2, Me, E(I()));
                                }, children: (Me, ft) => {
                                  var Ne = _a();
                                  let at;
                                  var Ht = d(Ne, true);
                                  s(Ne), L((Wt) => {
                                    at = Se(Ne, 1, "onclick nowrap svelte-12u4ifk", null, at, Wt), se(Ht, t(D).content);
                                  }, [() => ({ muted: t(D).muted })]), l(Me, Ne);
                                }, $$slots: { default: true } });
                              }, ut = (Re) => {
                                var Me = ba();
                                let ft;
                                var Ne = d(Me, true);
                                s(Me), L((at) => {
                                  ft = Se(Me, 1, "rawText nowrap svelte-12u4ifk", null, ft, at), se(Ne, t(D).content);
                                }, [() => ({ muted: t(D).muted })]), l(Re, Me);
                              };
                              j(Le, (Re) => {
                                t(D).onClick ? Re(We) : Re(ut, false);
                              }, Be);
                            }
                          };
                          j(ke, (Le) => {
                            var _a2;
                            ((_a2 = e.columns[U]) == null ? void 0 : _a2.showAs) === "check" ? Le(rt) : Le(Oe, false);
                          }, He);
                        }
                      };
                      j(me, (ke) => {
                        var _a2;
                        ((_a2 = e.columns[U]) == null ? void 0 : _a2.showAs) === "copyToClip" ? ke(Ze) : ke(tt, false);
                      }, Ue);
                    }
                  };
                  j(je, (me) => {
                    var _a2;
                    ((_a2 = e.columns[U]) == null ? void 0 : _a2.showAs) === "a_blank" ? me(dt) : me(yt, false);
                  }, ct);
                }
              };
              j(kt, (je) => {
                var _a2;
                ((_a2 = e.columns[U]) == null ? void 0 : _a2.showAs) === "a" ? je(Ve) : je(vt, false);
              });
            }
            s(ze), l(pe, ze);
          };
          j(de, (pe) => {
            r()[e.select ? U + 1 : U] && pe(we);
          });
        }
        l(N, $);
      });
      var ne = M(J);
      {
        var ge = (N) => {
          var D = ka(), U = d(D);
          it(U, { ariaLabel: "Options", btnInvisible: true, get offsetLeft() {
            return e.offsetLeftOptions;
          }, get offsetTop() {
            return e.offsetTopOptions;
          }, get close() {
            return t(R)[I()];
          }, set close(de) {
            t(R)[I()] = de;
          }, button: (de) => {
            var we = ma(), pe = d(we);
            aa(pe, {}), s(we), l(de, we);
          }, children: (de, we) => {
            var pe = ue(), ze = ee(pe);
            De(ze, () => e.options, T, () => t(R)[I()]), l(de, pe);
          }, $$slots: { button: true, default: true } }), s(D), l(N, D);
        };
        j(ne, (N) => {
          e.options && r()[r().length - 1] && N(ge);
        });
      }
      s(Y), L((N, D) => {
        ie = Se(Y, 1, "svelte-12u4ifk", null, ie, N), G = Ae(Y, "", G, { "grid-template-columns": D });
      }, [() => ({ highlight: t(g) === I() }), m]), l(h, Y);
    };
    var Rt = d(et);
    {
      var Mt = (h) => {
        var T = ue(), I = ee(T);
        {
          var Y = (ie) => {
            var G = ue(), re = ee(G);
            Ce(re, 17, () => e.rows, Te, (ce, J, ne) => {
              c(ce, () => t(J), () => ne);
            }), l(ie, G);
          };
          j(I, (ie) => {
            t(S).length === e.rows.length && ie(Y);
          });
        }
        l(h, T);
      }, zt = (h) => {
        var T = ue(), I = ee(T);
        Ce(I, 17, () => t(q), Te, (Y, ie, G) => {
          c(Y, () => t(ie), () => G);
        }), l(h, T);
      };
      j(Rt, (h) => {
        t(F) ? h(Mt) : h(zt, false);
      });
    }
    s(et), Qe(et, (h) => x(te, h), () => t(te));
  }
  var bt = M(et), wt = d(bt), mt = d(wt);
  const At = le(() => `-${u.length * 1.4 + 3}rem`);
  it(mt, { ariaLabel: "Select Columns", get offsetTop() {
    return t(At);
  }, btnInvisible: true, button: (h) => {
    var T = xa(), I = d(T);
    $t(I, {}), s(T), l(h, T);
  }, children: (h, T) => {
    var I = Pa(), Y = d(I);
    {
      var ie = (J) => {
        var ne = pa(), ge = d(ne);
        Ye(ge, { ariaLabel: "Select Column: Select", get checked() {
          return r()[0];
        }, set checked(N) {
          r(r()[0] = N, true);
        }, children: (N, D) => {
          lt();
          var U = ot("Select");
          l(N, U);
        }, $$slots: { default: true } }), s(ne), l(J, ne);
      };
      j(Y, (J) => {
        e.select && J(ie);
      });
    }
    var G = M(Y, 2);
    Ce(G, 17, () => e.columns, Te, (J, ne, ge) => {
      var N = Sa(), D = d(N);
      const U = le(() => `Select Column: ${t(ne).content}`);
      Ye(D, { get ariaLabel() {
        return t(U);
      }, get checked() {
        return r()[e.select ? ge + 1 : ge];
      }, set checked($) {
        r(r()[e.select ? ge + 1 : ge] = $, true);
      }, children: ($, de) => {
        lt();
        var we = ot();
        L(() => se(we, t(ne).content)), l($, we);
      }, $$slots: { default: true } }), s(N), l(J, N);
    });
    var re = M(G, 2);
    {
      var ce = (J) => {
        var ne = Ca(), ge = d(ne);
        Ye(ge, { ariaLabel: "Select Column: Options", get checked() {
          return r()[r().length - 1];
        }, set checked(N) {
          r(r()[r().length - 1] = N, true);
        }, children: (N, D) => {
          lt();
          var U = ot("Options");
          l(N, U);
        }, $$slots: { default: true } }), s(ne), l(J, ne);
      };
      j(re, (J) => {
        e.options && J(ce);
      });
    }
    s(I), l(h, I);
  }, $$slots: { button: true, default: true } });
  var st = M(mt, 2), qt = d(st);
  De(qt, () => e.caption ?? nt), s(st);
  var Et = M(st, 2);
  {
    var jt = (c) => {
      var h = La();
      l(c, h);
    }, Ut = (c) => {
      Nr(c, { get items() {
        return e.rows;
      }, get compact() {
        return i();
      }, get itemsPaginated() {
        return t(q);
      }, set itemsPaginated(h) {
        x(q, h, true);
      }, get page() {
        return t(v);
      }, set page(h) {
        x(v, h, true);
      }, get pageSize() {
        return t(w);
      }, set pageSize(h) {
        x(w, h, true);
      } });
    };
    j(Et, (c) => {
      t(F) ? c(jt) : c(Ut, false);
    });
  }
  s(wt), s(bt), s(X), L((c) => {
    f(X, "aria-colcount", u.length), f(X, "aria-rowcount", t(H)), Ee = Ae(X, "", Ee, { width: e.width, "max-width": e.maxWidth }), Ie = Ae(Je, "", Ie, { "grid-template-columns": c });
  }, [m]), l(a, X), be();
}
qe(["mousedown"]);
var Ia = k("<p>no results</p>");
function Oa(a, e) {
  _e(e, true);
  let r = A(oe([])), i = A(oe([]));
  ve(() => {
    let v = [], w = [];
    if (e.rows.length > 0) {
      for (let O of e.rows[0].columns) v.push({ content: O.name, initialWidth: "12rem", orderType: n(O.value) });
      for (let O of e.rows) {
        let S = [];
        for (let Q of O.columns) S.push({ content: y(Q.value) });
        w.push(S);
      }
    }
    x(r, v, true), x(i, w, true);
  });
  function o(v) {
    return [...new Uint8Array(v)].map((w) => w.toString(16).padStart(2, "0")).join("");
  }
  function n(v) {
    return v.hasOwnProperty("Integer") || v.hasOwnProperty("Real") ? "number" : "string";
  }
  function y(v) {
    return v.hasOwnProperty("Integer") ? v.Integer : v.hasOwnProperty("Real") ? v.Real : v.hasOwnProperty("Text") ? v.Text : v.hasOwnProperty("Blob") ? `x'${o(v.Blob)}'` : "NULL";
  }
  var _ = ue(), C = ee(_);
  {
    var u = (v) => {
      Da(v, { get columns() {
        return t(r);
      }, paginationPageSize: 100, get rows() {
        return t(i);
      }, set rows(w) {
        x(i, w, true);
      } });
    }, z = (v) => {
      var w = Ia();
      l(v, w);
    };
    j(C, (v) => {
      t(r).length > 0 && t(i).length > 0 ? v(u) : v(z, false);
    });
  }
  l(a, _), be();
}
function Ba(a, e) {
  a.ctrlKey && a.code === "Enter" && e();
}
var Ra = k('<div role="textbox" tabindex="0" class="query svelte-1o8x9h5" contenteditable=""></div>'), Ma = k('<div class="err"> </div>'), za = k('<!> <!> <div id="query-results" class="svelte-1o8x9h5"><!></div>', 1);
function Aa(a, e) {
  _e(e, true);
  let r = b(e, "query", 7), i = A(oe([])), o = A(""), n = A(void 0), y = A(void 0), _ = le(() => t(n) && t(y) ? `${t(n) - t(y)}px` : "100%");
  ve(() => {
    r().query.startsWith(xt) && (r().query = r().query.replace(`${xt}
`, ""), C());
  });
  async function C() {
    x(i, [], true), x(o, "");
    let R = [];
    for (let F of r().query.split(/\r?\n/)) F.startsWith("--") || R.push(F);
    let K = R.join(`
`), q = await tr("/query", K);
    if (q.status === 200) x(i, await q.json(), true);
    else {
      let F = await q.json();
      x(o, Object.values(F)[0], true);
    }
  }
  async function u(R) {
    x(y, R, true);
  }
  var z = za(), v = ee(z);
  er(v, { resizeBottom: true, minHeightPx: 100, initialHeightPx: 300, onResizeBottom: u, children: (R, K) => {
    var q = Ra();
    q.__keydown = [Ba, C], dr("innerText", q, () => r().query, (F) => r().query = F), l(R, q);
  }, $$slots: { default: true } });
  var w = M(v, 2);
  {
    var O = (R) => {
      var K = Ma(), q = d(K, true);
      s(K), L(() => se(q, t(o))), l(R, K);
    };
    j(w, (R) => {
      t(o) && R(O);
    });
  }
  var S = M(w, 2);
  let Q;
  var B = d(S);
  Oa(B, { get rows() {
    return t(i);
  }, set rows(R) {
    x(i, R, true);
  } }), s(S), L(() => Q = Ae(S, "", Q, { height: t(_), "max-height": t(_) })), ur("innerHeight", (R) => x(n, R, true)), l(a, z), be();
}
qe(["keydown"]);
var qa = ye('<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12"></path></svg>');
function Ea(a, e) {
  let r = b(e, "color", 8, "var(--col-err)"), i = b(e, "opacity", 8, 0.9), o = b(e, "width", 8, "1.5rem");
  var n = qa();
  f(n, "stroke-width", 2), L(() => {
    f(n, "width", o()), f(n, "color", r()), f(n, "opacity", i());
  }), l(a, n);
}
function ja(a, e, r, i) {
  t(e) ? a.code === "Enter" && (a.preventDefault(), r()) : i();
}
function St(a, e, r) {
  e.onClose(r());
}
var Ua = k('<div class="row svelte-1ml8s23"><div role="button" tabindex="0"><!></div> <div class="close svelte-1ml8s23"><div role="button" tabindex="0" class="close-inner svelte-1ml8s23"><!></div></div></div>');
function Ha(a, e) {
  _e(e, true);
  let r = b(e, "tab", 15), i = b(e, "tabSelected", 15), o, n = le(() => i() === r());
  function y() {
    let S = o.innerText;
    r(S), i(S);
  }
  function _() {
    t(n) || i(r());
  }
  var C = Ua(), u = d(C);
  u.__click = _, u.__keydown = [ja, n, y, _];
  var z = d(u);
  De(z, () => e.children), s(u), Qe(u, (S) => o = S, () => o);
  var v = M(u, 2), w = d(v);
  w.__click = [St, e, r], w.__keydown = [St, e, r];
  var O = d(w);
  Ea(O, { color: "hsl(var(--error))", width: "1.2rem" }), s(w), s(v), s(C), L(() => {
    Se(u, 1, rr(t(n) ? "tab selected" : "tab"), "svelte-1ml8s23"), f(u, "contenteditable", t(n));
  }), gt("blur", u, y), l(a, C), be();
}
qe(["click", "keydown"]);
var Wa = ye('<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15"></path></svg>');
function Na(a, e) {
  let r = b(e, "opacity", 8, 0.9), i = b(e, "width", 8, "1.5rem");
  var o = Wa();
  f(o, "stroke-width", 2), L(() => {
    f(o, "width", i()), f(o, "opacity", r());
  }), l(a, o);
}
function Ct() {
  ae.push({ id: Xe(6), query: ar });
}
var Qa = k('<div id="tabs" class="svelte-ko98zn"><!> <div role="button" tabindex="0" title="Add New Tab" class="ctrl add-new svelte-ko98zn"><!></div></div> <!>', 1);
function Fa(a, e) {
  _e(e, true);
  let r = A(oe(ae[0].id)), i = le(() => ae.filter((v) => v.id === t(r))[0]);
  ve(() => {
    ae.length > 0 ? x(r, ae[ae.length - 1].id, true) : x(r, "");
  });
  function o(v) {
    let O = ae.map((S) => S.id).indexOf(v);
    t(r) === v ? ae.length === 1 ? (ae.push(or), ae.shift(), x(r, ae[0].id, true)) : O === 0 ? (ae.shift(), x(r, ae[0].id, true)) : (ae.splice(O, 1), x(r, ae[O - 1].id, true)) : ae.splice(O, 1);
  }
  var n = Qa(), y = ee(n), _ = d(y);
  Ce(_, 17, () => ae, (v) => v.id, (v, w, O) => {
    Ha(v, { onClose: o, get tab() {
      return t(w).id;
    }, set tab(S) {
      t(w).id = S;
    }, get tabSelected() {
      return t(r);
    }, set tabSelected(S) {
      x(r, S, true);
    }, children: (S, Q) => {
      lt();
      var B = ot();
      L(() => se(B, t(w).id)), l(S, B);
    }, $$slots: { default: true } });
  });
  var C = M(_, 2);
  C.__click = [Ct], C.__keydown = [Ct];
  var u = d(C);
  Na(u, {}), s(C), s(y);
  var z = M(y, 2);
  Aa(z, { get query() {
    return t(i);
  } }), l(a, n), be();
}
qe(["click", "keydown"]);
var Va = k('<meta property="description" content="Hiqlite Dashboard">');
function $a(a) {
  Vt((e) => {
    var r = Va();
    Gt.title = "Hiqlite", l(e, r);
  }), Fa(a, {});
}
export {
  $a as component
};
