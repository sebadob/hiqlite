import{S as q,o as C,b as E,d as m,f as F,g as O,U as f,h as j,i as w,j as G,k as H,m as J,p as L,t as Q,v as I,w as U,H as S,x as T,y as V,z as P,A as _,B as $,C as k,D as W,E as X,F as Y,L as Z,P as rr,G as er,I as tr,J as ar,K as nr,a as sr,M as z,N as ir,O as vr,Q as ur}from"./runtime.Blj131-X.js";function D(v,c=null,y){if(typeof v!="object"||v===null||q in v)return v;const p=J(v);if(p!==C&&p!==E)return v;var n=new Map,u=L(v),g=m(0);u&&n.set("length",m(v.length));var o;return new Proxy(v,{defineProperty(t,r,e){(!("value"in e)||e.configurable===!1||e.enumerable===!1||e.writable===!1)&&F();var a=n.get(r);return a===void 0?(a=m(e.value),n.set(r,a)):O(a,D(e.value,o)),!0},deleteProperty(t,r){var e=n.get(r);return e===void 0?r in t&&n.set(r,m(f)):(O(e,f),A(g)),!0},get(t,r,e){var d;if(r===q)return v;var a=n.get(r),s=r in t;if(a===void 0&&(!s||(d=j(t,r))!=null&&d.writable)&&(a=m(D(s?t[r]:f,o)),n.set(r,a)),a!==void 0){var i=w(a);return i===f?void 0:i}return Reflect.get(t,r,e)},getOwnPropertyDescriptor(t,r){var e=Reflect.getOwnPropertyDescriptor(t,r);if(e&&"value"in e){var a=n.get(r);a&&(e.value=w(a))}else if(e===void 0){var s=n.get(r),i=s==null?void 0:s.v;if(s!==void 0&&i!==f)return{enumerable:!0,configurable:!0,value:i,writable:!0}}return e},has(t,r){var i;if(r===q)return!0;var e=n.get(r),a=e!==void 0&&e.v!==f||Reflect.has(t,r);if(e!==void 0||G!==null&&(!a||(i=j(t,r))!=null&&i.writable)){e===void 0&&(e=m(a?D(t[r],o):f),n.set(r,e));var s=w(e);if(s===f)return!1}return a},set(t,r,e,a){var N;var s=n.get(r),i=r in t;if(u&&r==="length")for(var d=e;d<s.v;d+=1){var R=n.get(d+"");R!==void 0?O(R,f):d in t&&(R=m(f),n.set(d+"",R))}s===void 0?(!i||(N=j(t,r))!=null&&N.writable)&&(s=m(void 0),O(s,D(e,o)),n.set(r,s)):(i=s.v!==f,O(s,D(e,o)));var h=Reflect.getOwnPropertyDescriptor(t,r);if(h!=null&&h.set&&h.set.call(a,e),!i){if(u&&typeof r=="string"){var x=n.get("length"),b=Number(r);Number.isInteger(b)&&b>=x.v&&O(x,b+1)}A(g)}return!0},ownKeys(t){w(g);var r=Reflect.ownKeys(t).filter(s=>{var i=n.get(s);return i===void 0||i.v!==f});for(var[e,a]of n)a.v!==f&&!(e in t)&&r.push(e);return r},setPrototypeOf(){H()}})}function A(v,c=1){O(v,v.v+c)}function lr(v,c,y,p=null,n=!1){I&&U();var u=v,g=null,o=null,t=null,r=n?X:0;Q(()=>{if(t===(t=!!c()))return;let e=!1;if(I){const a=u.data===S;t===a&&(u=T(),V(u),P(!1),e=!0)}t?(g?_(g):g=$(()=>y(u)),o&&k(o,()=>{o=null})):(o?_(o):p&&(o=$(()=>p(u))),g&&k(g,()=>{g=null})),e&&P(!0)},r),I&&(u=W)}function or(v,c,y,p){var N;var n=(y&tr)!==0,u=(y&ar)!==0,g=(y&nr)!==0,o=(y&ur)!==0,t=v[c],r=(N=j(v,c))==null?void 0:N.set,e=p,a=!0,s=()=>(o&&a&&(a=!1,e=sr(p)),e);t===void 0&&p!==void 0&&(r&&u&&Y(),t=s(),r&&r(t));var i;if(u)i=()=>{var l=v[c];return l===void 0?s():(a=!0,l)};else{var d=(n?z:ir)(()=>v[c]);d.f|=Z,i=()=>{var l=w(d);return l!==void 0&&(e=void 0),l===void 0?e:l}}if(!(y&rr))return i;if(r){var R=v.$$legacy;return function(l,K){return arguments.length>0?((!u||!K||R)&&r(K?i():l),l):i()}}var h=!1,x=vr(t),b=z(()=>{var l=i(),K=w(x);return h?(h=!1,K):x.v=l});return n||(b.equals=er),function(l,K){var B=w(b);if(arguments.length>0){const M=K?w(b):u&&g?D(l):l;return b.equals(M)||(h=!0,O(x,M),w(b)),l}return B}}export{D as a,lr as i,or as p};