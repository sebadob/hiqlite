import{k as x,t as S,l as f,i as h,j as p,m as L,v as R,g as M,w as F,x as H,y as $,q as G,z as J,$ as V}from"../chunks/disclose-version.DdsrvPbs.js";import{K as j,x as A,a as C,N as E,g as b,z as w,Q as T,M as U,G as W}from"../chunks/runtime.6J5aEZU7.js";import{e as P,i as K,b as X,f as Z,r as ee,A as Y,s as k,a as te,d as re,c as ae,Q as u,g as se,D as ie,h as oe}from"../chunks/randomKey.CvWxylyc.js";import{a as _,p as q}from"../chunks/props.DDMHF-id.js";import{b as ne}from"../chunks/this.32KUBlWl.js";var le=S('<div class="value svelte-1m6c23m"><span class="svelte-1m6c23m"> </span></div>'),de=S('<div class="col svelte-1m6c23m"><div class="head svelte-1m6c23m"><b class="svelte-1m6c23m"> </b></div> <!></div>'),ce=S('<div id="query-results" class="svelte-1m6c23m"></div>');function ve(o,t){j(t,!0);let e=A(_([[]]));C(()=>{if(t.rows.length===0){w(e,_([[]]));return}let r=[];for(let c=0;c<t.rows[0].columns.length;c++)r.push([]);for(let c of t.rows){let l=c.columns;for(let s=0;s<l.length;s++)r[s].push(l[s])}w(e,_(r))});function d(r){return[...new Uint8Array(r)].map(c=>c.toString(16).padStart(2,"0")).join("")}function n(r){return r.hasOwnProperty("Integer")?r.Integer:r.hasOwnProperty("Real")?r.Real:r.hasOwnProperty("Text")?r.Text:r.hasOwnProperty("Blob")?`x'${d(r.Blob)}'`:"NULL"}var a=ce();P(a,73,()=>b(e),K,(r,c,l)=>{var s=de(),m=h(s),i=h(m),y=h(i);f(i),f(m);var g=p(p(m,!0));P(g,65,()=>T(c),K,(v,O,Q)=>{var D=le(),I=h(D),z=h(I);U(()=>L(z,n(T(O).value))),f(I),f(D),x(v,D)}),f(s),U(()=>{var v;return L(y,(v=T(c)[0])==null?void 0:v.name)}),x(r,s)}),f(a),x(o,a),E()}function ue(o,t){o.ctrlKey&&o.code==="Enter"&&t()}var fe=S(`<textarea name="query" class="svelte-m4ye7i">
</textarea> <!>`,1);function _e(o,t){j(t,!0);let e=q(t,"query",7),d=A(_([]));C(()=>{e().query.startsWith(Y)&&n()});async function n(){w(d,_([]));let l=[];for(let i of e().query.split(/\r?\n/))i.startsWith("--")||l.push(i);let s=l.join(`
`),m=await Z("/query",s);m.status===200?w(d,_(await m.json())):console.error(await m.json())}var a=fe(),r=M(a);ee(r),r.__keydown=[ue,n];var c=p(p(r,!0));ve(c,{get rows(){return b(d)},set rows(l){w(d,_(l))}}),X(r,()=>e().query,l=>e().query=l),x(o,a),E()}R(["keydown"]);var he=F('<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12"></path></svg>');function ye(o,t){let e=q(t,"color",0,"var(--col-err)"),d=q(t,"opacity",0,.9),n=q(t,"width",0,20);var a=he();k(a,"stroke-width",2),h(a),f(a),U(()=>{k(a,"width",n()),k(a,"color",e()),k(a,"opacity",d())}),x(o,a)}function me(o,t,e,d){b(t)?o.code==="Enter"&&(o.preventDefault(),e()):d()}function N(o,t,e){t.onClose(e())}var we=S('<div class="row svelte-19lyo5o"><div role="button" tabindex="0"><!></div> <div class="close svelte-19lyo5o"><div role="button" tabindex="0" class="close-inner svelte-19lyo5o"><!></div></div></div>');function pe(o,t){j(t,!0);let e=q(t,"tab",7),d=q(t,"tabSelected",7),n,a=W(()=>d()===e());function r(){let v=n.innerText;e(v),d(v)}function c(){b(a)||d(e())}var l=we(),s=h(l);ne(s,v=>n=v,()=>n),s.__click=c,s.__keydown=[me,a,r,c];var m=h(s);te(m,re(t),{}),f(s);var i=p(p(s,!0)),y=h(i);y.__click=[N,t,e],y.__keydown=[N,t,e];var g=h(y);ye(g,{}),f(y),f(i),f(l),U(()=>{ae(s,`${(b(a)?"tab selected":"tab")??""} svelte-19lyo5o`),k(s,"contenteditable",b(a))}),H("blur",s,r,!1),x(o,l),E()}R(["click","keydown"]);var be=F('<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15"></path></svg>');function xe(o,t){let e=q(t,"opacity",0,.9),d=q(t,"width",0,24);var n=be();k(n,"stroke-width",2),h(n),f(n),U(()=>{k(n,"width",d()),k(n,"opacity",e())}),x(o,n)}function B(){u.push({id:se(6),query:ie})}var ge=S('<div id="tabs" class="svelte-9cpjlp"><!> <div role="button" tabindex="0" title="Add New Tab" class="ctrl add-new svelte-9cpjlp"><!></div></div> <!>',1);function ke(o,t){j(t,!0);let e=A(_(u[0].id)),d=W(()=>u.filter(i=>i.id===b(e))[0]);C(()=>{let i=u[u.length-1];i!=null&&i.query.startsWith(Y)&&w(e,_(i.id))});function n(i){let g=u.map(v=>v.id).indexOf(i);b(e)===i?u.length===1?(u.push(oe),u.shift(),w(e,_(u[0].id))):g===0?(u.shift(),w(e,_(u[0].id))):(u.splice(g,1),w(e,_(u[g-1].id))):u.splice(g,1)}var a=ge(),r=M(a),c=h(r);P(c,69,()=>u,(i,y)=>i.id,(i,y,g)=>{pe(i,{get tab(){return T(y).id},set tab(v){T(y).id=v},get tabSelected(){return b(e)},set tabSelected(v){w(e,_(v))},onClose:n,children:(v,O)=>{$();var Q=G();U(()=>L(Q,T(y).id)),x(v,Q)},$$slots:{default:!0}})});var l=p(p(c,!0));l.__click=[B],l.__keydown=[B];var s=h(l);xe(s,{}),f(l),f(r);var m=p(p(r,!0));_e(m,{get query(){return b(d)}}),x(o,a),E()}R(["click","keydown"]);var qe=S('<meta property="description" content="Hiqlite Dashboard">');function Qe(o){J(t=>{var e=qe();V.title="Hiqlite",x(t,e)}),ke(o,{$$legacy:!0})}export{Qe as component};