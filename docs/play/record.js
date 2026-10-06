import {initialState,legalMoves,isGameOver,pass,parseMove,applyMove,formatMove} from './engine/rules.js';
export function normalize(state){return !legalMoves(state).length&&!isGameOver(state)?pass(state):state;}
export function parseSequence(text){
 const states=[initialState()],moves=[];
 for(const token of text.trim().split(/\s+/).filter(Boolean)){
  try{const s=states.at(-1);if(isGameOver(s)||token.toLowerCase()==='pass')throw Error();const m=parseMove(s,token);moves.push(formatMove(m));states.push(normalize(applyMove(s,m)));}catch{throw Error(String(moves.length+1));}
 }
 return {states,moves};
}
