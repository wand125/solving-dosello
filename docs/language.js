export function resolveLanguage(search,stored){const value=new URLSearchParams(search).get('lang');return ['en','ja'].includes(value)?value:stored==='ja'?'ja':'en';}
export function languageController(update){
 let stored;try{stored=localStorage.getItem('dosello-language');}catch{}
 let lang=resolveLanguage(location.search,stored);
 const apply=()=>{document.documentElement.lang=lang;for(const b of document.querySelectorAll('[data-language]'))b.setAttribute('aria-pressed',String(b.dataset.language===lang));update(lang);};
 for(const b of document.querySelectorAll('[data-language]'))b.onclick=()=>{lang=b.dataset.language;try{localStorage.setItem('dosello-language',lang);}catch{}const url=new URL(location.href);url.searchParams.set('lang',lang);history.replaceState(null,'',url);apply();};
 apply();return ()=>lang;
}
