import {original} from '../../test/original-helper.js';
if(!original){console.error('SKIP: optional original unavailable');process.exit(0);}
import {initialState,toOriginal} from '../../engine/rules.js';
function perft({board,shape,turn},d){if(d===0)return 1;let ms=original.moves(board,turn,shape);if(!ms.length)return original.moves(board,-turn,shape).length?perft({board,shape,turn:-turn},d-1):1;let n=0;for(const m of ms)n+=perft({board:original.apply(board,m,turn),shape:original.nextShape(shape,m),turn:-turn},d-1);return n;}
const depth=+(process.argv[2]??3),start=performance.now();console.log(JSON.stringify({depth,leaves:perft(toOriginal(initialState()),depth),elapsedMs:performance.now()-start}));
