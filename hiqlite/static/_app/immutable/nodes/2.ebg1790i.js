import{a as s,t as g,c as le,k as Fe,j as ce,e as je,s as te,b as De,l as wt}from"../chunks/disclose-version.DzBAV3dJ.js";import{R as ot,p as ve,j as d,k as i,t as _,g as t,i as de,f as G,u as ne,h as F,a6 as U,m as I,v as k,a as Ze,n as mt,aG as He,aF as kt}from"../chunks/runtime.5e4YztcE.js";import{n as yt,s as $e,a as u,o as rt,l as Re,B as he,r as lt,g as we,i as ze,d as Le,b as nt,p as xt,I as $t,q as zt,m as st,u as Lt,j as St,Q as J,v as Ct,D as _t}from"../chunks/genKey.BWJn_pnY.js";import{c as Tt,p as f,b as qe,i as E,a as y}from"../chunks/props.Lh6i5LpO.js";import{s as jt,a as Mt,p as Pt}from"../chunks/stores.Be4_cH-m.js";function at(l,e,o){if(l.multiple)return It(l,e);for(var a of l.options){var r=Be(a);if(Tt(r,e)){a.selected=!0;return}}(!o||e!==void 0)&&(l.selectedIndex=-1)}function Ot(l,e){ot(()=>{var o=new MutationObserver(()=>{var a=l.__value;at(l,a)});return o.observe(l,{childList:!0,subtree:!0,attributes:!0,attributeFilter:["value"]}),()=>{o.disconnect()}})}function At(l,e,o=e){var a=!0;yt(l,"change",()=>{var r;if(l.multiple)r=[].map.call(l.querySelectorAll(":checked"),Be);else{var n=l.querySelector(":checked");r=n&&Be(n)}o(r)}),ot(()=>{var r=e();if(at(l,r,a),a&&r===void 0){var n=l.querySelector(":checked");n!==null&&(r=Be(n),o(r))}l.__value=r,a=!1}),Ot(l)}function It(l,e){for(var o of l.options)o.selected=~e.indexOf(Be(o))}function Be(l){return"__value"in l?l.__value:l.value}async function Rt(l){var e;await((e=navigator==null?void 0:navigator.clipboard)==null?void 0:e.writeText(l))}var qt=g('<span class="font-label"><a class="svelte-a0xtvp"><!></a></span>');function it(l,e){ve(e,!0);const o=jt(),a=()=>Mt(Pt,"$page",o);let r=f(e,"selectedStep",3,!1),n=f(e,"hideUnderline",3,!1),m=G(()=>{if(r())return"step";if(a().route.id===e.href.split("?")[0])return"page"});var v=qt(),T=d(v),c=d(T);$e(c,()=>e.children),i(T),i(v),_(()=>{u(T,"href",e.href),u(T,"target",e.target),u(T,"aria-current",t(m)),rt(T,"hideUnderline",n())}),s(l,v),de()}var Bt=g('<!> <div class="popover svelte-1au8ouo" popover="auto"><div class="inner fade-in svelte-1au8ouo"><!></div></div>',1);function Ve(l,e){ve(e,!0);let o=f(e,"ref",15),a=f(e,"roleButton",3,"button"),r=f(e,"offsetLeft",3,"0px"),n=f(e,"offsetTop",3,"0px"),m=f(e,"close",15);const v=Re(8),T=Re(8);let c=U(void 0),S=U(!1);ne(()=>{m(h)});function w(){if(o()&&t(c))if(e.absolute)t(c).style.top=n(),t(c).style.left=r();else{let P=o().getBoundingClientRect();t(c).style.top=`calc(${P.bottom+window.scrollY}px + ${n()})`,t(c).style.left=`calc(${P.left+window.scrollX}px + ${r()})`}else console.error("button and popover ref missing")}function h(){var P;(P=t(c))==null||P.hidePopover()}function M(P){var R;let C=P.newState;k(S,C==="open"),(R=e.onToggle)==null||R.call(e,C)}var O=Bt(),D=F(O);he(D,{get ref(){return o()},set ref(P){o(P)},get role(){return a()},id:v,ariaControls:T,popovertarget:T,onclick:w,get invisible(){return e.btnInvisible},get isDisabled(){return e.btnDisabled},get onLeft(){return e.onLeft},get onRight(){return e.onRight},get onUp(){return e.onUp},get onDown(){return e.onDown},children:(P,C)=>{var R=le(),q=F(R);$e(q,()=>e.button),s(P,R)},$$slots:{default:!0}});var B=I(D,2);qe(B,P=>k(c,P),()=>t(c)),u(B,"id",T),u(B,"aria-labelledby",v);var oe=d(B),ue=d(oe);E(ue,()=>e.lazy,P=>{var C=le(),R=F(C);E(R,()=>t(S),q=>{var x=le(),L=F(x);$e(L,()=>e.children),s(q,x)}),s(P,C)},P=>{var C=le(),R=F(C);$e(R,()=>e.children),s(P,C)}),i(oe),i(B),_(()=>u(B,"aria-label",e.ariaLabel)),Fe("toggle",B,M),s(l,O),de()}var Ut=ce('<svg fill="none" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M12 9.75L14.25 12m0 0l2.25 2.25M14.25 12l2.25-2.25M14.25 12L12 14.25m-2.58 4.92l-6.375-6.375a1.125 1.125 0 010-1.59L9.42 4.83c.211-.211.498-.33.796-.33H19.5a2.25 2.25 0 012.25 2.25v10.5a2.25 2.25 0 01-2.25 2.25h-9.284c-.298 0-.585-.119-.796-.33z"></path></svg>');function Et(l,e){let o=f(e,"color",8,"currentColor"),a=f(e,"opacity",8,.9),r=f(e,"width",8,20);var n=Ut();u(n,"stroke-width",2),_(()=>{u(n,"stroke",o()),u(n,"width",r()),u(n,"opacity",a())}),s(l,n)}var Dt=ce('<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M21 21l-5.197-5.197m0 0A7.5 7.5 0 105.196 5.196a7.5 7.5 0 0010.607 10.607z"></path></svg>');function Zt(l,e){let o=f(e,"opacity",8,.9),a=f(e,"width",8,20);var r=Dt();u(r,"stroke-width",2),_(()=>{u(r,"width",a()),u(r,"opacity",o())}),s(l,r)}function Ht(l,e,o,a){var r,n,m;switch(l.code){case"Enter":e();break;case"Tab":(r=o.onTab)==null||r.call(o,a());break;case"ArrowUp":(n=o.onUp)==null||n.call(o,a());break;case"ArrowDown":(m=o.onDown)==null||m.call(o,a());break}}var Vt=g('<div class="options svelte-13lxusw"><!></div>'),Nt=g("<option></option>"),Wt=g('<datalist class="absolute svelte-13lxusw"></datalist>'),Xt=g('<div class="magnify svelte-13lxusw"><!></div>'),Kt=g('<div class="btnSearch svelte-13lxusw"><!></div>'),Qt=g('<search class="flex container svelte-13lxusw"><!> <input type="search" autocomplete="off" aria-label="Search" placeholder="Search" class="svelte-13lxusw"> <!> <div class="relative"><div class="absolute btnDelete svelte-13lxusw"><!></div></div> <!></search>');function Yt(l,e){ve(e,!0);let o=f(e,"value",15,""),a=f(e,"option",15),r=f(e,"focus",15),n=f(e,"width",3,"100%");const m=Re(8),v=Re(8);let T=U(void 0),c=G(()=>e.datalist&&e.datalist.length>0?v:void 0);ne(()=>{r(w)});function S(){var C;(C=e.onSearch)==null||C.call(e,o())}function w(){var C;(C=t(T))==null||C.focus()}var h=Qt(),M=d(h);E(M,()=>e.options,C=>{var R=Vt(),q=d(R);ct(q,{ariaLabel:"Search Options",get options(){return e.options},get value(){return a()},set value(x){a(x)},borderless:!0}),i(R),s(C,R)});var O=I(M,2);qe(O,C=>k(T,C),()=>t(T)),lt(O),u(O,"id",m),O.__keydown=[Ht,S,e,o];var D=I(O,2);E(D,()=>e.datalist,C=>{var R=Wt();u(R,"id",v),we(R,21,()=>e.datalist,ze,(q,x)=>{var L=Nt(),$={};_(()=>{$!==($=t(x))&&(L.value=(L.__value=t(x))==null?"":t(x))}),s(q,L)}),i(R),s(C,R)});var B=I(D,2),oe=d(B),ue=d(oe);he(ue,{ariaLabel:"Delete Search Input",invisible:!0,onclick:()=>o(""),children:(C,R)=>{Et(C,{color:"hsl(var(--bg-high))",width:24})},$$slots:{default:!0}}),i(oe),i(B);var P=I(B,2);E(P,()=>e.onSearch,C=>{var R=Kt(),q=d(R);he(q,{ariaLabel:"Search",invisible:!0,onclick:S,children:(x,L)=>{var $=Xt(),b=d($);Zt(b,{}),i($),s(x,$)},$$slots:{default:!0}}),i(R),s(C,R)}),i(h),_(()=>{Le(h,"border",e.borderless?void 0:"1px solid hsl(var(--bg-high))"),Le(h,"width",n()),u(O,"list",t(c))}),Fe("focus",O,()=>{var C;return(C=e.onFocus)==null?void 0:C.call(e)}),nt(O,o),s(l,h),de()}je(["keydown"]);var Ft=ce('<svg fill="none" viewBox="0 0 24 24" color="currentColor" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M8.25 4.5l7.5 7.5-7.5 7.5"></path></svg>');function Ge(l,e){let o=f(e,"opacity",8,.9),a=f(e,"width",8,20);var r=Ft();u(r,"stroke-width",2),_(()=>{u(r,"width",a()),u(r,"opacity",o())}),s(l,r)}function Gt(l,e,o,a,r){let n=l.code;n==="ArrowDown"?(l.preventDefault(),e()&&k(o,t(o)+1)):n==="ArrowUp"?(l.preventDefault(),e()&&k(o,t(o)-1)):n==="Enter"&&t(o)>-1?a(t(r)[t(o)]):n==="Enter"&&t(o)===-1&&t(r).length===1&&a(t(r)[0])}var Jt=g('<div class="btn svelte-1j5gmms"> <div class="chevron svelte-1j5gmms"><!></div></div>'),eo=g('<div class="optPopover svelte-1j5gmms"> </div>'),to=g('<div role="listbox" tabindex="0" class="popover svelte-1j5gmms"><!> <div class="popoverOptions svelte-1j5gmms"></div></div>'),oo=g('<option class="opt svelte-1j5gmms"> </option>'),ro=g('<select class="svelte-1j5gmms"></select>');function ct(l,e){ve(e,!0);let o=f(e,"ref",15),a=f(e,"options",19,()=>[]),r=f(e,"value",15),n=f(e,"asPopover",3,!0),m=f(e,"borderless",3,!1),v=f(e,"withSearch",3,!1),T=f(e,"fallbackOptions",3,!1),c=U(void 0),S=U(y(T()?!1:n())),w=U(void 0),h=U(y(v()?-1:0)),M=U(void 0),O=U(""),D=G(()=>{if(!v())return a();if(typeof r()=="string")return a().filter(x=>x.toLowerCase().includes(t(O).toLowerCase()));let q=Number.parseInt(t(O))||r();return a().filter(x=>x===q)});ne(()=>{t(S)!==n()&&k(S,y(n()))}),ne(()=>{var q,x;if(t(h)===-1&&((q=t(c))==null||q.scrollTo({top:0,behavior:"smooth"})),v()){if(t(h)<0||t(h)>t(D).length-1){k(h,-1),(x=t(M))==null||x();return}}else t(h)<0?k(h,t(D).length-1):t(h)>t(D).length-1&&k(h,0),B()});function B(){if(t(c)){let q=t(c).getElementsByTagName("button")[t(h)];q.scrollIntoView({behavior:"smooth",block:"center"}),q.focus()}else console.error("refOptions is undefined")}function oe(q){var x;q==="open"&&(v()?(k(h,-1),(x=t(M))==null||x()):(k(h,y(a().findIndex(L=>L===r())||0)),B()))}function ue(){return t(D).length>0?!0:(k(h,-1),!1)}function P(q){r(q),k(O,""),setTimeout(()=>{var x;(x=t(w))==null||x()},20)}var C=le(),R=F(C);E(R,()=>t(S),q=>{Ve(q,{get ref(){return o()},set ref(x){o(x)},get ariaLabel(){return e.ariaLabel},roleButton:"combobox",btnInvisible:!0,get close(){return t(w)},set close(x){k(w,y(x))},get offsetTop(){return e.offsetTop},get offsetLeft(){return e.offsetLeft},onToggle:oe,get onLeft(){return e.onLeft},get onRight(){return e.onRight},get onUp(){return e.onUp},get onDown(){return e.onDown},button:x=>{var L=Jt(),$=d(L),b=I($),W=d(b);Ge(W,{width:14}),i(b),i(L),_(()=>{u(L,"data-border",!m()),te($,`${r()??""} `)}),s(x,L)},children:(x,L)=>{var $=to();$.__keydown=[Gt,ue,h,P,D];var b=d($);E(b,v,Q=>{Yt(Q,{get value(){return t(O)},set value(Z){k(O,y(Z))},get focus(){return t(M)},set focus(Z){k(M,y(Z))},onFocus:()=>k(h,-1)})});var W=I(b,2);qe(W,Q=>k(c,Q),()=>t(c)),we(W,21,()=>t(D),ze,(Q,Z,ge)=>{he(Q,{invisible:!0,invisibleOutline:!0,onclick:()=>P(t(Z)),children:(be,Ne)=>{var se=eo(),Me=d(se);i(se),_(()=>{u(se,"aria-selected",r()===t(Z)),u(se,"data-focus",t(h)===ge),te(Me,t(Z))}),s(be,se)},$$slots:{default:!0}})}),i(W),i($),_(()=>Le($,"max-height",e.maxHeight)),s(x,$)},$$slots:{button:!0,default:!0}})},q=>{var x=ro();we(x,21,()=>t(D),ze,(L,$)=>{var b=oo(),W={},Q=d(b);i(b),_(()=>{W!==(W=t($))&&(b.value=(b.__value=t($))==null?"":t($)),b.selected=r()===t($),te(Q,t($))}),s(L,b)}),i(x),_(()=>{u(x,"name",e.name),u(x,"aria-label",e.ariaLabel),rt(x,"borderless",m())}),At(x,r),s(q,x)}),s(l,C),de()}je(["keydown"]);var lo=g('<div class="link noselect svelte-1bye1t3"> </div>'),no=g('<li class="svelte-1bye1t3"><!></li>'),so=g('<nav aria-label="Pagination" class="svelte-1bye1t3"><ul class="svelte-1bye1t3"></ul></nav>'),ao=g('<div class="flex gap-10"><div class="flex gap-05 chunkSize noselect svelte-1bye1t3"><div>Entries</div> <div><!></div></div> <div class="font-label total svelte-1bye1t3"> </div></div>'),io=g('<div class="iconLeft svelte-1bye1t3" aria-label="Go to previous page"><!></div>'),co=g('<div class="iconRight svelte-1bye1t3" aria-label="Go to next page"><!></div>'),vo=g('<div class="container svelte-1bye1t3"><!> <!> <!> <!></div>');function uo(l,e){ve(e,!0);const o=L=>{var $=so(),b=d($);we(b,21,()=>t(h),ze,(W,Q)=>{var Z=no(),ge=d(Z);he(ge,{invisible:!0,onclick:()=>D(t(Q)),onLeft:M,onRight:O,children:(be,Ne)=>{var se=lo(),Me=d(se);i(se),_(()=>te(Me,t(Q))),s(be,se)},$$slots:{default:!0}}),i(Z),_(()=>{u(Z,"aria-label",`go to page number: ${t(Q)}`),u(Z,"aria-current",m()===t(Q)?"step":void 0)}),s(W,Z)}),i(b),i($),s(L,$)},a=L=>{var $=ao(),b=d($),W=I(d(b),2),Q=d(W);ct(Q,{ariaLabel:"Page Count",get value(){return v()},set value(be){v(be)},options:r,offsetTop:"-17rem",borderless:!0}),i(W),i(b);var Z=I(b,2),ge=d(Z);i(Z),i($),_(()=>te(ge,`Total: ${e.items.length??""}`)),s(L,$)},r=[5,7,10,15,20,30,50,100];let n=f(e,"itemsPaginated",15),m=f(e,"page",15,1),v=f(e,"pageSize",31,()=>y(r[0])),T=f(e,"compact",3,!1);const c=16;let S=Ze(()=>v()),w=U(y([])),h=U(y([]));ne(()=>{v()!==S&&(S=Ze(()=>v()),m(1))}),ne(()=>{let L=[];for(let $=0;$<e.items.length;$+=v()){const b=e.items.slice($,$+v());L.push(b)}k(w,y(L)),n(L[m()-1])}),ne(()=>{B()});function M(){m()>1&&D(m()-1)}function O(){m()<t(w).length&&D(m()+1)}function D(L){m(L),B()}function B(){let L=[],$=Math.floor(v()/2);if(t(w).length<=v())for(let b=1;b<=t(w).length;(b+=1)-1)L.push(b);else if(m()<=$)for(let b=1;b<=v();(b+=1)-1)L.push(b);else if(m()>t(w).length-$-1)for(let b=t(w).length-v();b<=t(w).length-1;(b+=1)-1)L.push(b+1);else for(let b=m()-$;b<m()-$+v();(b+=1)-1)L.push(b);k(h,y(L))}var oe=vo(),ue=d(oe),P=G(()=>m()===1);he(ue,{onclick:M,invisible:!0,get isDisabled(){return t(P)},children:(L,$)=>{var b=io(),W=d(b);Ge(W,{width:c}),i(b),_(()=>u(b,"data-disabled",m()===1)),s(L,b)},$$slots:{default:!0}});var C=I(ue,2);o(C);var R=I(C,2),q=G(()=>m()===t(w).length);he(R,{onclick:O,invisible:!0,get isDisabled(){return t(q)},children:(L,$)=>{var b=co(),W=d(b);Ge(W,{width:c}),i(b),_(()=>u(b,"data-disabled",m()===t(w).length)),s(L,b)},$$slots:{default:!0}});var x=I(R,2);E(x,()=>!T(),L=>{a(L)}),i(oe),s(l,oe),de()}function po(l,e){console.log(l.code),l.code==="Enter"&&e()}var fo=g('<label class="font-label noselect svelte-1supmpl"><input type="checkbox" class="svelte-1supmpl"> <span class="svelte-1supmpl"><!></span></label>');function Ue(l,e){ve(e,!0);let o=f(e,"checked",15,!1),a=f(e,"ariaLabel",3,""),r=f(e,"borderColor",3,"hsl(var(--bg-high))");function n(){o(!o())}var m=fo(),v=d(m);lt(v),v.__click=n,v.__keydown=[po,n];var T=I(v,2),c=d(T);$e(c,()=>e.children??mt),i(T),i(m),_(()=>{u(v,"name",e.name),v.disabled=e.disabled,u(v,"aria-disabled",e.disabled),u(v,"aria-checked",o()),u(v,"aria-label",a()),Le(v,"border-color",r())}),xt(v,o),s(l,m),de()}je(["click","keydown"]);var ho=ce('<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m19.5 8.25-7.5 7.5-7.5-7.5"></path></svg>');function go(l,e){let o=f(e,"opacity",8,.9),a=f(e,"width",8,20);var r=ho();u(r,"stroke-width",2),u(r,"color",color),_(()=>{u(r,"width",a()),u(r,"opacity",o())}),s(l,r)}var bo=ce('<svg fill="none" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M3 7.5 7.5 3m0 0L12 7.5M7.5 3v13.5m13.5 0L16.5 21m0 0L12 16.5m4.5 4.5V7.5"></path></svg>');function wo(l,e){let o=f(e,"color",8,"currentColor"),a=f(e,"opacity",8,.9),r=f(e,"width",8,20);var n=bo();u(n,"stroke-width",2),_(()=>{u(n,"stroke",o()),u(n,"width",r()),u(n,"opacity",a())}),s(l,n)}var mo=ce(`<svg fill="none" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213
            1.281c.063.374.313.686.645.87.074.04.147.083.22.127.325.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 0
            1 1.37.49l1.296 2.247a1.125 1.125 0 0 1-.26 1.431l-1.003.827c-.293.241-.438.613-.43.992a7.723 7.723 0 0
            1 0 .255c-.008.378.137.75.43.991l1.004.827c.424.35.534.955.26 1.43l-1.298 2.247a1.125 1.125 0 0
            1-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.47 6.47 0 0
            1-.22.128c-.331.183-.581.495-.644.869l-.213 1.281c-.09.543-.56.94-1.11.94h-2.594c-.55 0-1.019-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52
            0 0 1-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 0 1-1.369-.49l-1.297-2.247a1.125
            1.125 0 0 1 .26-1.431l1.004-.827c.292-.24.437-.613.43-.991a6.932 6.932 0 0 1 0-.255c.007-.38-.138-.751-.43-.992l-1.004-.827a1.125 1.125
            0 0 1-.26-1.43l1.297-2.247a1.125 1.125 0 0 1 1.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.086.22-.128.332-.183.582-.495.644-.869l.214-1.28Z"></path><path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z"></path></svg>`);function ko(l,e){let o=f(e,"color",8,"currentColor"),a=f(e,"opacity",8,.9),r=f(e,"width",8,20);var n=mo();u(n,"stroke-width",2),_(()=>{u(n,"stroke",o()),u(n,"width",r()),u(n,"opacity",a())}),s(l,n)}var yo=ce('<svg fill="none" viewBox="0 0 24 24" stroke="currentColor" color="hsl(var(--action))"><path stroke-linecap="round" stroke-linejoin="round" d="M4.5 12.75l6 6 9-13.5"></path></svg>'),xo=ce('<svg fill="none" viewBox="0 0 24 24" stroke="currentColor" color="hsl(var(--error))"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12"></path></svg>');function $o(l,e){var o=le(),a=F(o);E(a,()=>e.checked,r=>{var n=yo();u(n,"stroke-width",2),u(n,"width",20),u(n,"opacity",.9),s(r,n)},r=>{var n=xo();u(n,"stroke-width",2),u(n,"width",20),u(n,"opacity",.9),s(r,n)}),s(l,o)}var zo=ce(`<svg fill="none" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M6.75 12a.75.75 0 1 1-1.5 0 .75.75 0 0 1 1.5 0ZM12.75 12a.75.75 0 1 1-1.5 0 .75.75 0 0 1 1.5
            0ZM18.75 12a.75.75 0 1 1-1.5 0 .75.75 0 0 1 1.5 0Z"></path></svg>`);function Lo(l,e){let o=f(e,"color",8,"currentColor"),a=f(e,"opacity",8,.9),r=f(e,"width",8,20);var n=zo();u(n,"stroke-width",2),_(()=>{u(n,"stroke",o()),u(n,"width",r()),u(n,"opacity",a())}),s(l,n)}var So=g('<span class="btnSelect svelte-i6noez"><!></span>'),Co=g('<th class="headerCheckbox svelte-i6noez"><!> <!></th>'),_o=g('<span class="iconOrder svelte-i6noez"><!></span>'),To=g(" <!>",1),jo=g('<span class="rawText svelte-i6noez"> </span>'),Mo=g('<th class="svelte-i6noez"><span class="flex-1 label svelte-i6noez"><!></span> <span class="relative"><span role="none" class="absolute sizeable svelte-i6noez"></span></span></th>'),Po=g('<th class="headerOptions svelte-i6noez"><!></th>'),Oo=g('<td class="checkbox svelte-i6noez"><!></td>'),Ao=g('<span class="linkText svelte-i6noez"> </span>'),Io=g('<span class="linkText svelte-i6noez"> </span>'),Ro=g('<span class="copyToClip svelte-i6noez"> </span>'),qo=g('<span class="checkIcon svelte-i6noez"><!></span>'),Bo=g('<span class="onclick svelte-i6noez"> </span>'),Uo=g('<span class="rawText svelte-i6noez"> </span>'),Eo=g('<td class="svelte-i6noez"><!></td>'),Do=g('<span class="btnOptions svelte-i6noez"><!></span>'),Zo=g('<td class="options svelte-i6noez"><!></td>'),Ho=g('<tr class="svelte-i6noez"><!><!><!></tr>'),Vo=g('<span class="eye svelte-i6noez"><!></span>'),No=g('<div class="columnSelect svelte-i6noez"><!></div>'),Wo=g('<div class="columnSelect svelte-i6noez"><!></div>'),Xo=g('<div class="columnSelect svelte-i6noez"><!></div>'),Ko=g('<div class="columnsSelect svelte-i6noez"><!> <!> <!></div>'),Qo=g('<table class="svelte-i6noez"><thead class="svelte-i6noez"><tr class="svelte-i6noez"><!><!><!></tr></thead><tbody class="svelte-i6noez"></tbody><caption class="flex space-between svelte-i6noez"><!> <span class="flex gap-05 svelte-i6noez"><span> </span> <!></span></caption></table>');function Yo(l,e){ve(e,!0);let o=f(e,"showColumns",31,()=>y(Array(e.columns.length).fill(!0))),a=f(e,"paginationCompact",3,!1),r=f(e,"minWidthColPx",3,50);const n="3rem",m="2rem";let v=y(R()),T=U(y(Ze(()=>v))),c=U(1),S=U(15),w=U(y(Array(e.rows.length).fill(!1))),h=G(()=>t(w).find(p=>p===!0)),M=U(void 0),O=U(y([])),D=U(!1),B=U(y([])),oe=U("up"),ue=G(()=>t(B)&&t(B).length?t(B).length:(t(c)!=1&&k(c,1),0)),P=y(Array(Ze(()=>v.length)).fill(void 0)),C=0;setTimeout(()=>{for(let p=1;p<=v.length;(p+=1)-1)if(v[p]==="auto"){C=p-1;let z=P[p];z&&Q(z.getBoundingClientRect().width)}},1e3),ne(()=>{let p=Array(e.rows.length).fill(!1);if(t(D)){let z;t(c)===1?z=0:z=(t(c)-1)*t(S);let A=Math.min(t(c)*t(S),e.rows.length);for(let V=z;V<A;(V+=1)-1)p[V]=!0}k(w,y(p))}),ne(()=>{var p;k(O,y(Array((p=t(B))==null?void 0:p.length).fill(()=>console.error("un-initialized popover close option"))))}),ne(()=>{let p=[];for(let z=0;z<v.length;(z+=1)-1)o()[z]&&p.push(v[z]);k(T,y(p))});function R(){let p=e.columns.map(z=>z.initialWidth);return e.select&&(p=[n,...p]),e.options&&(p=[...p,m]),o(Array(p.length).fill(!0)),p}function q(){return t(T).join(" ")}function x(p,z){k(w,y(Array(e.rows.length).fill(!1)));let A=1;t(oe)==="up"?(A=-1,k(oe,"down")):k(oe,"up"),z==="string"?e.rows.sort((V,ae)=>V[p].content.localeCompare(ae[p].content)*A):z==="number"&&e.rows.sort((V,ae)=>(V[p].content-ae[p].content)*A)}function L(p){return t(c)>1?(t(c)-1)*t(S)+p:p}function $(p){C=p;let z=P[p];z?(Q(z.getBoundingClientRect().width),window.addEventListener("mousemove",W),window.addEventListener("mouseup",b,{once:!0})):console.error("invalid ref from refCols in onMouseDown")}function b(){window.removeEventListener("mousemove",W)}function W(p){let z=P[C];if(z){let A=z.getBoundingClientRect().left,V=window.scrollX+p.x-A;Q(V)}else console.error("invalid ref from refCols in onMove")}function Q(p){p=Math.ceil(p),p<r()&&(p=r()),v[e.select?C+1:C]=`${p}px`}var Z=Qo(),ge=d(Z),be=d(ge);const Ne=G(q);_(()=>Le(be,"grid-template-columns",t(Ne)));var se=d(be);E(se,()=>e.select&&o()[0],p=>{var z=Co(),A=d(z);Ue(A,{ariaLabel:"Select All",get checked(){return t(D)},set checked(re){k(D,y(re))},borderColor:"hsla(var(--text), .4)"});var V=I(A,2),ae=G(()=>!t(h));Ve(V,{ariaLabel:"Options for the selection",get close(){return t(M)},set close(re){k(M,y(re))},get btnDisabled(){return t(ae)},btnInvisible:!0,button:re=>{var X=So(),N=d(X);go(N,{width:18}),i(X),_(()=>u(X,"data-disabled",!t(h))),s(re,X)},children:(re,X)=>{var N=le(),Y=F(N);$e(Y,()=>e.select,()=>t(w),()=>t(M)),s(re,N)},$$slots:{button:!0,default:!0}}),i(z),s(p,z)});var Me=I(se);we(Me,17,()=>e.columns,ze,(p,z,A)=>{var V=le(),ae=F(V);E(ae,()=>o()[e.select?A+1:A],re=>{var X=Mo();qe(X,(H,K)=>P[K]=H,H=>P==null?void 0:P[H],()=>[A]);var N=d(X),Y=d(N);E(Y,()=>t(z).orderType,H=>{var K=To(),ie=F(K),pe=I(ie);he(pe,{invisible:!0,onclick:()=>x(A,t(z).orderType),children:(Ke,Se)=>{var me=_o(),Oe=d(me);wo(Oe,{width:16}),i(me),s(Ke,me)},$$slots:{default:!0}}),_(()=>te(ie,`${t(z).content??""} `)),s(H,K)},H=>{var K=jo(),ie=d(K);i(K),_(()=>te(ie,t(z).content)),s(H,K)}),i(N);var j=I(N,2),ee=d(j);ee.__mousedown=()=>$(A),i(j),i(X),s(re,X)}),s(p,V)});var ut=I(Me);E(ut,()=>e.options&&o()[o().length-1],p=>{var z=Po(),A=d(z);ko(A,{width:20}),i(z),s(p,z)}),i(be),i(ge);var We=I(ge);we(We,21,()=>t(B),ze,(p,z,A)=>{var V=Ho();const ae=G(q);_(()=>Le(V,"grid-template-columns",t(ae)));var re=d(V);E(re,()=>e.select&&o()[0],Y=>{var j=Oo(),ee=d(j);Ue(ee,{ariaLabel:"Select Row",get checked(){return t(w)[L(A)]},set checked(H){t(w)[L(A)]=H}}),i(j),s(Y,j)});var X=I(re);we(X,17,()=>t(z),ze,(Y,j,ee)=>{var H=le(),K=F(H);E(K,()=>o()[e.select?ee+1:ee],ie=>{var pe=Eo(),Ke=d(pe);E(Ke,()=>{var Se;return((Se=e.columns[ee])==null?void 0:Se.showAs)==="a"},Se=>{var me=G(()=>t(j).href||"");it(Se,{get href(){return t(me)},children:(Oe,Ce)=>{var ke=Ao(),Ae=d(ke);i(ke),_(()=>te(Ae,t(j).content)),s(Oe,ke)},$$slots:{default:!0}})},Se=>{var me=le(),Oe=F(me);E(Oe,()=>{var Ce;return((Ce=e.columns[ee])==null?void 0:Ce.showAs)==="a_blank"},Ce=>{var ke=G(()=>t(j).href||"");it(Ce,{get href(){return t(ke)},target:"_blank",children:(Ae,_e)=>{var ye=Io(),Ee=d(ye);i(ye),_(()=>te(Ee,t(j).content)),s(Ae,ye)},$$slots:{default:!0}})},Ce=>{var ke=le(),Ae=F(ke);E(Ae,()=>{var _e;return((_e=e.columns[ee])==null?void 0:_e.showAs)==="copyToClip"},_e=>{he(_e,{invisible:!0,onclick:()=>Rt(t(j).content.toString()),children:(ye,Ee)=>{var fe=Ro(),xe=d(fe);i(fe),_(()=>te(xe,t(j).content)),s(ye,fe)},$$slots:{default:!0}})},_e=>{var ye=le(),Ee=F(ye);E(Ee,()=>{var fe;return((fe=e.columns[ee])==null?void 0:fe.showAs)==="check"},fe=>{var xe=qo(),Qe=d(xe);$o(Qe,{get checked(){return t(j).content}}),i(xe),s(fe,xe)},fe=>{var xe=le(),Qe=F(xe);E(Qe,()=>t(j).onClick,Ye=>{he(Ye,{invisible:!0,onclick:Te=>{var Ie,Pe;return(Pe=(Ie=t(j)).onClick)==null?void 0:Pe.call(Ie,Te,L(A))},children:(Te,Ie)=>{var Pe=Bo(),bt=d(Pe);i(Pe),_(()=>te(bt,t(j).content)),s(Te,Pe)},$$slots:{default:!0}})},Ye=>{var Te=Uo(),Ie=d(Te);i(Te),_(()=>te(Ie,t(j).content)),s(Ye,Te)},!0),s(fe,xe)},!0),s(_e,ye)},!0),s(Ce,ke)},!0),s(Se,me)}),i(pe),s(ie,pe)}),s(Y,H)});var N=I(X);E(N,()=>e.options&&o()[o().length-1],Y=>{var j=Zo(),ee=d(j);Ve(ee,{ariaLabel:"Options",get close(){return t(O)[A]},set close(H){t(O)[A]=H},btnInvisible:!0,get offsetLeft(){return e.offsetLeftOptions},get offsetTop(){return e.offsetTopOptions},button:H=>{var K=Do(),ie=d(K);Lo(ie,{}),i(K),s(H,K)},children:(H,K)=>{var ie=le(),pe=F(ie);$e(pe,()=>e.options,()=>t(z),()=>t(O)[A]),s(H,ie)},$$slots:{button:!0,default:!0}}),i(j),s(Y,j)}),i(V),s(p,V)}),i(We);var Je=I(We),et=d(Je);uo(et,{get items(){return e.rows},get itemsPaginated(){return t(B)},set itemsPaginated(p){k(B,y(p))},get page(){return t(c)},set page(p){k(c,y(p))},get pageSize(){return t(S)},set pageSize(p){k(S,y(p))},get compact(){return a()}});var tt=I(et,2),Xe=d(tt),pt=d(Xe);i(Xe);var ft=I(Xe,2),ht=G(()=>e.offsetLeftColumnSelect||"-6rem"),gt=G(()=>`-${v.length*1.4+2.7}rem`);Ve(ft,{ariaLabel:"Select Columns",get offsetLeft(){return t(ht)},get offsetTop(){return t(gt)},btnInvisible:!0,button:p=>{var z=Vo(),A=d(z);$t(A,{}),i(z),s(p,z)},children:(p,z)=>{var A=Ko(),V=d(A);E(V,()=>e.select,X=>{var N=No(),Y=d(N);Ue(Y,{ariaLabel:"Select Columns: Select",get checked(){return o()[0]},set checked(j){o(o()[0]=j,!0)},children:(j,ee)=>{He();var H=De("Select");s(j,H)},$$slots:{default:!0}}),i(N),s(X,N)});var ae=I(V,2);we(ae,17,()=>e.columns,ze,(X,N,Y)=>{var j=Wo(),ee=d(j),H=G(()=>`Select Columns: ${t(N).content}`);Ue(ee,{get ariaLabel(){return t(H)},get checked(){return o()[e.select?Y+1:Y]},set checked(K){o(o()[e.select?Y+1:Y]=K,!0)},children:(K,ie)=>{He();var pe=De();_(()=>te(pe,t(N).content)),s(K,pe)},$$slots:{default:!0}}),i(j),s(X,j)});var re=I(ae,2);E(re,()=>e.options,X=>{var N=Xo(),Y=d(N);Ue(Y,{ariaLabel:"Select Columns: Options",get checked(){return o()[o().length-1]},set checked(j){o(o()[o().length-1]=j,!0)},children:(j,ee)=>{He();var H=De("Options");s(j,H)},$$slots:{default:!0}}),i(N),s(X,N)}),i(A),s(p,A)},$$slots:{button:!0,default:!0}}),i(tt),i(Je),i(Z),_(()=>{u(Z,"aria-colcount",v.length),u(Z,"aria-rowcount",t(ue)),Le(Z,"width",e.width),Le(Z,"max-width",e.maxWidth),te(pt,e.caption)}),s(l,Z),de()}je(["mousedown"]);var Fo=g("<p>no results</p>"),Go=g('<div id="query-results" class="svelte-1sixxab"><!></div>');function Jo(l,e){ve(e,!0);let o=U(y([])),a=U(y([]));ne(()=>{let c=[],S=[];if(e.rows.length>0){for(let w of e.rows[0].columns)c.push({content:w.name,initialWidth:"12rem",orderType:n(w.value)});for(let w of e.rows){let h=[];for(let M of w.columns)h.push({content:m(M.value)});S.push(h)}}k(o,y(c)),k(a,y(S))});function r(c){return[...new Uint8Array(c)].map(S=>S.toString(16).padStart(2,"0")).join("")}function n(c){return c.hasOwnProperty("Integer")||c.hasOwnProperty("Real")?"number":"string"}function m(c){return c.hasOwnProperty("Integer")?c.Integer:c.hasOwnProperty("Real")?c.Real:c.hasOwnProperty("Text")?c.Text:c.hasOwnProperty("Blob")?`x'${r(c.Blob)}'`:"NULL"}var v=Go(),T=d(v);E(T,()=>t(o).length>0&&t(a).length>0,c=>{Yo(c,{get columns(){return t(o)},get rows(){return t(a)},set rows(S){k(a,y(S))}})},c=>{var S=Fo();s(c,S)}),i(v),s(l,v),de()}function er(l,e){l.ctrlKey&&l.code==="Enter"&&e()}var tr=g('<div class="err"> </div>'),or=g(`<textarea name="query" class="svelte-1jxlw0c">
    </textarea> <!> <!>`,1);function rr(l,e){ve(e,!0);let o=f(e,"query",7),a=U(y([])),r=U("");ne(()=>{o().query.startsWith(st)&&(o().query=o().query.replace(`${st}
`,""),n())});async function n(){k(a,y([])),k(r,"");let S=[];for(let M of o().query.split(/\r?\n/))M.startsWith("--")||S.push(M);let w=S.join(`
`),h=await zt("/query",w);if(h.status===200)k(a,y(await h.json()));else{let M=await h.json();k(r,y(Object.values(M)[0]))}}var m=or(),v=F(m);Lt(v),v.__keydown=[er,n];var T=I(v,2);E(T,()=>t(r),S=>{var w=tr(),h=d(w);i(w),_(()=>te(h,t(r))),s(S,w)});var c=I(T,2);Jo(c,{get rows(){return t(a)},set rows(S){k(a,y(S))}}),nt(v,()=>o().query,S=>o().query=S),s(l,m),de()}je(["keydown"]);var lr=ce('<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12"></path></svg>');function nr(l,e){let o=f(e,"color",8,"var(--col-err)"),a=f(e,"opacity",8,.9),r=f(e,"width",8,20);var n=lr();u(n,"stroke-width",2),_(()=>{u(n,"width",r()),u(n,"color",o()),u(n,"opacity",a())}),s(l,n)}function sr(l,e,o,a){t(e)?l.code==="Enter"&&(l.preventDefault(),o()):a()}function vt(l,e,o){e.onClose(o())}var ar=g('<div class="row svelte-1ml8s23"><div role="button" tabindex="0"><!></div> <div class="close svelte-1ml8s23"><div role="button" tabindex="0" class="close-inner svelte-1ml8s23"><!></div></div></div>');function ir(l,e){ve(e,!0);let o=f(e,"tab",15),a=f(e,"tabSelected",15),r,n=G(()=>a()===o());function m(){let O=r.innerText;o(O),a(O)}function v(){t(n)||a(o())}var T=ar(),c=d(T);qe(c,O=>r=O,()=>r),c.__click=v,c.__keydown=[sr,n,m,v];var S=d(c);$e(S,()=>e.children),i(c);var w=I(c,2),h=d(w);h.__click=[vt,e,o],h.__keydown=[vt,e,o];var M=d(h);nr(M,{color:"hsl(var(--error))"}),i(h),i(w),i(T),_(()=>{St(c,`${(t(n)?"tab selected":"tab")??""} svelte-1ml8s23`),u(c,"contenteditable",t(n))}),Fe("blur",c,m),s(l,T),de()}je(["click","keydown"]);var cr=ce('<svg fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15"></path></svg>');function vr(l,e){let o=f(e,"opacity",8,.9),a=f(e,"width",8,24);var r=cr();u(r,"stroke-width",2),_(()=>{u(r,"width",a()),u(r,"opacity",o())}),s(l,r)}function dt(){J.push({id:Re(6),query:Ct})}var dr=g('<div id="tabs" class="svelte-ko98zn"><!> <div role="button" tabindex="0" title="Add New Tab" class="ctrl add-new svelte-ko98zn"><!></div></div> <!>',1);function ur(l,e){ve(e,!0);let o=U(y(J[0].id)),a=G(()=>J.filter(w=>w.id===t(o))[0]);ne(()=>{J.length>0?k(o,y(J[J.length-1].id)):k(o,"")});function r(w){let h=J.map(M=>M.id).indexOf(w);t(o)===w?J.length===1?(J.push(_t),J.shift(),k(o,y(J[0].id))):h===0?(J.shift(),k(o,y(J[0].id))):(J.splice(h,1),k(o,y(J[h-1].id))):J.splice(h,1)}var n=dr(),m=F(n),v=d(m);we(v,17,()=>J,w=>w.id,(w,h)=>{ir(w,{get tab(){return t(h).id},set tab(M){t(h).id=M},get tabSelected(){return t(o)},set tabSelected(M){k(o,y(M))},onClose:r,children:(M,O)=>{He();var D=De();_(()=>te(D,t(h).id)),s(M,D)},$$slots:{default:!0}})});var T=I(v,2);T.__click=[dt],T.__keydown=[dt];var c=d(T);vr(c,{}),i(T),i(m);var S=I(m,2);rr(S,{get query(){return t(a)}}),s(l,n),de()}je(["click","keydown"]);var pr=g('<meta property="description" content="Hiqlite Dashboard">');function fr(l){wt(e=>{var o=pr();kt.title="Hiqlite",s(e,o)}),ur(l,{})}export{fr as component};
