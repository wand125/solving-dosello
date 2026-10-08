// D4 canonical keys shared by the TOOL and DISPLAY books.
import {fromOriginal} from '../engine/rules.js';
export function cell(a,t){let r=a>>3,c=a%8;if(t>=4)c=7-c;for(let i=0;i<t%4;i++)[r,c]=[c,7-r];return r*8+c;}
export function canonical(position){
 const s=fromOriginal(position);let best,tBest;
 for(let t=0;t<8;t++){
  const bits=[0n,0n,0n,0n];
  for(let a=0;a<64;a++)if(s.board[a]){const b=cell(a,t),partner=cell(s.pair[a],t);bits[s.board[a]===1?0:1]|=1n<<BigInt(b);if(b<partner)bits[partner-b===1?2:3]|=1n<<BigInt(b);}
  if(!best||bits.some((b,i)=>b<best[i]&&bits.slice(0,i).every((v,j)=>v===best[j]))){best=bits;tBest=t;}
 }
 return {bits:best,side:s.turn,transform:tBest};
}
