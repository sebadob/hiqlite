const __vite__mapDeps=(i,m=__vite__mapDeps,d=(m.f||(m.f=["../nodes/0.vPWvSk9r.js","../chunks/disclose-version.CvGloQaN.js","../chunks/runtime.BuBXzGL_.js","../chunks/misc.CT6z90nz.js","../nodes/1.m4RY4KvX.js","../chunks/render.DKi3Cjyk.js","../chunks/entry.flpNJx67.js","../nodes/2.DYPIbPJZ.js","../chunks/props.BwSdl-5L.js","../chunks/index-client.BwM6KDdC.js","../chunks/ThemeSwitchAbsolute.Cjp5mgkD.js","../assets/ThemeSwitchAbsolute.CYPjjtwa.css","../assets/2.Q62QUMH7.css","../nodes/3.CNnRNzSr.js","../assets/3.C4DIdr6N.css"])))=>i.map(i=>d[i]);
var D=r=>{throw TypeError(r)};var I=(r,t,s)=>t.has(r)||D("Cannot "+s);var c=(r,t,s)=>(I(r,t,"read from private field"),s?s.call(r):t.get(r)),L=(r,t,s)=>t.has(r)?D("Cannot add the same private member more than once"):t instanceof WeakSet?t.add(r):t.set(r,s),O=(r,t,s,a)=>(I(r,t,"write to private field"),a?a.call(r,s):t.set(r,s),s);import{n as F,q as H,t as J,O as K,P as M,d as N,B as X,e as Z,g as E,s as x,Q as $,h as tt,z as et,p as rt,G as st,u as nt,R as at,a as ot,b as S,N as it}from"../chunks/runtime.BuBXzGL_.js";import{a as ct,m as ut,u as lt,s as ft}from"../chunks/render.DKi3Cjyk.js";import{h as V,d as dt,j as ht,f as k,c as A,a as b,s as j,t as G,b as mt,z as _t,r as vt}from"../chunks/disclose-version.CvGloQaN.js";import{p as T,a as gt,i as C}from"../chunks/props.BwSdl-5L.js";import{o as yt}from"../chunks/index-client.BwM6KDdC.js";function p(r,t,s){V&&dt();var a=r,o,e;F(()=>{o!==(o=t())&&(e&&(J(e),e=null),o&&(e=H(()=>s(a,o))))}),V&&(a=ht)}function z(r,t){var a;var s=r&&((a=r[Z])==null?void 0:a.t);return r===t||s===t}function B(r={},t,s,a){return K(()=>{var o,e;return M(()=>{o=e,e=[],N(()=>{r!==s(...e)&&(t(r,...e),o&&z(s(...o),r)&&t(null,...o))})}),()=>{X(()=>{e&&z(s(...e),r)&&t(null,...e)})}}),r}function bt(r){return class extends Et{constructor(t){super({component:r,...t})}}}var _,f;class Et{constructor(t){L(this,_);L(this,f);var s=new Map,a=(e,n)=>{var u=et(n);return s.set(e,u),u};const o=new Proxy({...t.props||{},$$events:{}},{get(e,n){return E(s.get(n)??a(n,Reflect.get(e,n)))},has(e,n){return E(s.get(n)??a(n,Reflect.get(e,n))),Reflect.has(e,n)},set(e,n,u){return x(s.get(n)??a(n,u),u),Reflect.set(e,n,u)}});O(this,f,(t.hydrate?ct:ut)(t.component,{target:t.target,props:o,context:t.context,intro:t.intro??!1,recover:t.recover})),$(),O(this,_,o.$$events);for(const e of Object.keys(c(this,f)))e==="$set"||e==="$destroy"||e==="$on"||tt(this,e,{get(){return c(this,f)[e]},set(n){c(this,f)[e]=n},enumerable:!0});c(this,f).$set=e=>{Object.assign(o,e)},c(this,f).$destroy=()=>{lt(c(this,f))}}$set(t){c(this,f).$set(t)}$on(t,s){c(this,_)[t]=c(this,_)[t]||[];const a=(...o)=>s.call(this,...o);return c(this,_)[t].push(a),()=>{c(this,_)[t]=c(this,_)[t].filter(o=>o!==a)}}$destroy(){c(this,f).$destroy()}}_=new WeakMap,f=new WeakMap;const Rt="modulepreload",kt=function(r,t){return new URL(r,t).href},U={},w=function(t,s,a){let o=Promise.resolve();if(s&&s.length>0){const e=document.getElementsByTagName("link"),n=document.querySelector("meta[property=csp-nonce]"),u=(n==null?void 0:n.nonce)||(n==null?void 0:n.getAttribute("nonce"));o=Promise.all(s.map(l=>{if(l=kt(l,a),l in U)return;U[l]=!0;const v=l.endsWith(".css"),P=v?'[rel="stylesheet"]':"";if(!!a)for(let d=e.length-1;d>=0;d--){const m=e[d];if(m.href===l&&(!v||m.rel==="stylesheet"))return}else if(document.querySelector(`link[href="${l}"]${P}`))return;const i=document.createElement("link");if(i.rel=v?"stylesheet":Rt,v||(i.as="script",i.crossOrigin=""),i.href=l,u&&i.setAttribute("nonce",u),document.head.appendChild(i),v)return new Promise((d,m)=>{i.addEventListener("load",d),i.addEventListener("error",()=>m(new Error(`Unable to preload CSS for ${l}`)))})}))}return o.then(()=>t()).catch(e=>{const n=new Event("vite:preloadError",{cancelable:!0});if(n.payload=e,window.dispatchEvent(n),!n.defaultPrevented)throw e})},Bt={};var wt=G('<div id="svelte-announcer" aria-live="assertive" aria-atomic="true" style="position: absolute; left: 0; top: 0; clip: rect(0 0 0 0); clip-path: inset(50%); overflow: hidden; white-space: nowrap; width: 1px; height: 1px"><!></div>'),xt=G("<!> <!>",1);function Pt(r,t){rt(t,!0);let s=T(t,"components",15,()=>[]),a=T(t,"data_0",3,null),o=T(t,"data_1",3,null);st(()=>t.stores.page.set(t.page)),nt(()=>{t.stores,t.page,t.constructors,s(),t.form,a(),o(),t.stores.page.notify()});let e=S(!1),n=S(!1),u=S(null);yt(()=>{const g=t.stores.page.subscribe(()=>{E(e)&&(x(n,!0),at().then(()=>{x(u,gt(document.title||"untitled page"))}))});return x(e,!0),g});var l=xt(),v=k(l);C(v,()=>t.constructors[1],g=>{var i=A(),d=k(i);p(d,()=>t.constructors[0],(m,y)=>{B(y(m,{get data(){return a()},children:(h,Lt)=>{var q=A(),Q=k(q);p(Q,()=>t.constructors[1],(W,Y)=>{B(Y(W,{get data(){return o()},get form(){return t.form}}),R=>s()[1]=R,()=>{var R;return(R=s())==null?void 0:R[1]})}),b(h,q)},$$slots:{default:!0}}),h=>s()[0]=h,()=>{var h;return(h=s())==null?void 0:h[0]})}),b(g,i)},g=>{var i=A(),d=k(i);p(d,()=>t.constructors[0],(m,y)=>{B(y(m,{get data(){return a()},get form(){return t.form}}),h=>s()[0]=h,()=>{var h;return(h=s())==null?void 0:h[0]})}),b(g,i)});var P=j(j(v,!0));C(P,()=>E(e),g=>{var i=wt(),d=mt(i);C(d,()=>E(n),m=>{var y=_t();it(()=>ft(y,E(u))),b(m,y)}),vt(i),b(g,i)}),b(r,l),ot()}const qt=bt(Pt),Dt=[()=>w(()=>import("../nodes/0.vPWvSk9r.js"),__vite__mapDeps([0,1,2,3]),import.meta.url),()=>w(()=>import("../nodes/1.m4RY4KvX.js"),__vite__mapDeps([4,1,2,5,6]),import.meta.url),()=>w(()=>import("../nodes/2.DYPIbPJZ.js"),__vite__mapDeps([7,1,2,5,8,9,10,11,12]),import.meta.url),()=>w(()=>import("../nodes/3.CNnRNzSr.js"),__vite__mapDeps([13,1,2,10,8,5,11,3,14]),import.meta.url)],It=[],Vt={"/":[2],"/login":[3]},jt={handleError:({error:r})=>{console.error(r)},reroute:()=>{}};export{Vt as dictionary,jt as hooks,Bt as matchers,Dt as nodes,qt as root,It as server_loads};