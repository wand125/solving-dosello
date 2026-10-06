use std::fmt;
const NOT_A: u64 = 0xfefefefefefefefe;
const NOT_H: u64 = 0x7f7f7f7f7f7f7f7f;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Position {
    pub black: u64,
    pub white: u64,
    pub hleft: u64,
    pub vtop: u64,
    pub side: i8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Move {
    pub a: u8,
    pub b: u8,
    pub flips: u64,
}
// At most 56 horizontal + 56 vertical placements on an 8x8 board.
#[derive(Clone)]
pub struct MoveList { data: [std::mem::MaybeUninit<Move>; 112], len: usize }
impl MoveList {
    fn new() -> Self { Self { data: [std::mem::MaybeUninit::uninit();112], len:0 } }
    fn push(&mut self, m: Move) { self.data[self.len].write(m); self.len+=1; }
}
impl std::ops::Deref for MoveList {
    type Target=[Move];
    fn deref(&self)->&[Move] { // SAFETY: push initializes exactly the first len entries; Move is Copy.
        unsafe {std::slice::from_raw_parts(self.data.as_ptr().cast(),self.len)} }
}
impl std::ops::DerefMut for MoveList {
    fn deref_mut(&mut self)->&mut [Move] { // SAFETY: only initialized entries are exposed, with exclusive access.
        unsafe {std::slice::from_raw_parts_mut(self.data.as_mut_ptr().cast(),self.len)} }
}
const fn rays() -> [[u64;8];64] {
    let mut out=[[0;8];64];
    let dr=[-1,-1,-1,0,0,1,1,1];
    let dc=[-1,0,1,-1,1,-1,0,1];
    let mut a=0;
    while a<64 { let mut d=0; while d<8 {
        let mut r=a as i32/8+dr[d]; let mut c=a as i32%8+dc[d];
        while r>=0 && r<8 && c>=0 && c<8 {out[a][d]|=1u64<<(r*8+c);r+=dr[d];c+=dc[d];}
        d+=1;
    } a+=1; } out
}
const RAYS:[[u64;8];64]=rays();
#[inline(always)]
fn cell_flips(a:usize,own:u64,opp:u64)->u64 {
    // Explicit independent directions let LLVM schedule/vectorize the anchors.
    #[inline(always)]
    fn backward(ray:u64,own:u64,opp:u64)->u64 {
        let blockers=ray & !opp;
        let anchor=1u64 << (63-(blockers|1).leading_zeros());
        (ray & anchor.wrapping_neg().wrapping_shl(1)) &
            0u64.wrapping_sub((anchor & own & ray != 0) as u64)
    }
    #[inline(always)]
    fn forward(ray:u64,own:u64,opp:u64)->u64 {
        let blockers=ray & !opp;
        let anchor=blockers & blockers.wrapping_neg();
        (ray & anchor.wrapping_sub(1)) & 0u64.wrapping_sub((anchor & own != 0) as u64)
    }
    let r=RAYS[a];
    backward(r[0],own,opp)|backward(r[1],own,opp)|backward(r[2],own,opp)|backward(r[3],own,opp)|
    forward(r[4],own,opp)|forward(r[5],own,opp)|forward(r[6],own,opp)|forward(r[7],own,opp)
}
impl Move {
    pub fn id(self) -> u16 {
        self.a as u16 * 64 + self.b as u16
    }
}
impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}-{}{}",
            (b'a' + self.a % 8) as char,
            self.a / 8 + 1,
            (b'a' + self.b % 8) as char,
            self.b / 8 + 1
        )
    }
}
#[inline(always)]
pub fn shift(x: u64, d: usize) -> u64 {
    match d {
        0 => (x & NOT_A) >> 9,
        1 => x >> 8,
        2 => (x & NOT_H) >> 7,
        3 => (x & NOT_A) >> 1,
        4 => (x & NOT_H) << 1,
        5 => (x & NOT_A) << 7,
        6 => x << 8,
        _ => (x & NOT_H) << 9,
    }
}
pub fn adjacent(x: u64) -> u64 {
    (0..8).fold(0, |a, d| a | shift(x, d))
}
pub fn placements(e: u64) -> i32 {
    ((e & (e >> 1) & NOT_H).count_ones() + (e & (e >> 8)).count_ones()) as i32
}
impl Position {
    pub fn initial() -> Self {
        Self {
            black: (1 << 20) | (1 << 28) | (1 << 35) | (1 << 43),
            white: (1 << 26) | (1 << 27) | (1 << 36) | (1 << 37),
            hleft: (1 << 26) | (1 << 36),
            vtop: (1 << 20) | (1 << 35),
            side: 1,
        }
    }
    pub fn empty(self) -> u64 {
        !(self.black | self.white)
    }
    pub fn diff(self) -> i32 {
        self.black.count_ones() as i32 - self.white.count_ones() as i32
    }
    pub fn pass(mut self) -> Self {
        self.side = -self.side;
        self
    }
    pub fn own(self) -> u64 {
        if self.side == 1 {
            self.black
        } else {
            self.white
        }
    }
    pub fn opp(self) -> u64 {
        if self.side == 1 {
            self.white
        } else {
            self.black
        }
    }
    #[inline]
    pub fn partners(self, x: u64) -> u64 {
        ((x & self.hleft) << 1)
            | ((x >> 1) & self.hleft)
            | ((x & self.vtop) << 8)
            | ((x >> 8) & self.vtop)
    }
    #[inline]
    pub fn capture(self, cells:u64)->u64 {
        let mut starts=cells; let mut direct=0;
        while starts!=0 {let a=starts.trailing_zeros() as usize; starts&=starts-1;direct|=cell_flips(a,self.own()|cells,self.opp());}
        direct | self.partners(direct)
    }
    /// Empty squares with a direct capture. Adjacent new domino cells cannot
    /// anchor one another across an opponent, so their captures are independent.
    #[inline]
    pub fn targets(self)->u64 {
        let own=self.own(); let opp=self.opp(); let mut out=0;
        for d in 0..8 {
            let mut x=shift(own,d)&opp;
            // A run has at most six opponents between its two endpoints.
            x|=shift(x,d)&opp; x|=shift(x,d)&opp; x|=shift(x,d)&opp;
            x|=shift(x,d)&opp; x|=shift(x,d)&opp;
            out|=shift(x,d);
        }
        out & self.empty()
    }
    #[inline]
    pub fn mobility(self)->i32 {
        let e=self.empty();let t=self.targets();
        ((e&(e>>1)&NOT_H&(t|(t>>1))).count_ones()+(e&(e>>8)&(t|(t>>8))).count_ones()) as i32
    }
    pub fn move_list(self)->MoveList {
        let e=self.empty(); let t=self.targets();
        let h=e&(e>>1)&NOT_H&(t|(t>>1));let v=e&(e>>8)&(t|(t>>8));
        let mut needed=(h|h<<1|v|v<<8)&t;
        let mut flips=[0u64;64];
        while needed!=0 {let a=needed.trailing_zeros() as usize;needed&=needed-1;
            let direct=cell_flips(a,self.own(),self.opp()); flips[a]=direct|self.partners(direct);
        }
        let mut starts=h|v;let mut out=MoveList::new();
        while starts!=0 {let a=starts.trailing_zeros() as u8;let bit=1u64<<a;starts&=starts-1;
            if h&bit!=0 {out.push(Move{a,b:a+1,flips:flips[a as usize]|flips[a as usize+1]});}
            if v&bit!=0 {out.push(Move{a,b:a+8,flips:flips[a as usize]|flips[a as usize+8]});}
        } out
    }
    /// Placements only, for leaf search and TT-first staging. Fill flips before play.
    pub fn lazy_moves(self)->MoveList {
        let e=self.empty();let t=self.targets();
        let h=e&(e>>1)&NOT_H&(t|(t>>1));let v=e&(e>>8)&(t|(t>>8));
        let mut starts=h|v;let mut out=MoveList::new();
        while starts!=0 {let a=starts.trailing_zeros() as u8;let bit=1u64<<a;starts&=starts-1;
            if h&bit!=0 {out.push(Move{a,b:a+1,flips:0});}
            if v&bit!=0 {out.push(Move{a,b:a+8,flips:0});}
        } out
    }
    #[inline]
    pub fn with_flips(self,mut m:Move)->Move {
        let direct=cell_flips(m.a as usize,self.own(),self.opp())|
                   cell_flips(m.b as usize,self.own(),self.opp());
        m.flips=direct|self.partners(direct);m
    }
    pub fn moves(self)->Vec<Move> { self.move_list().to_vec() }
    pub fn play(mut self, m: Move) -> Self {
        let bits = (1u64 << m.a) | (1u64 << m.b) | m.flips;
        if self.side == 1 {
            self.black |= bits;
            self.white &= !bits;
        } else {
            self.white |= bits;
            self.black &= !bits;
        }
        if m.b - m.a == 1 {
            self.hleft |= 1 << m.a
        } else {
            self.vtop |= 1 << m.a
        }
        self.side = -self.side;
        self
    }
    pub fn hash(self)->u64 {
        fingerprint(self.black,0)^fingerprint(self.white,1)^fingerprint(self.hleft,2)^fingerprint(self.vtop,3)^if self.side==1 {0} else {SIDE_KEY}
    }
    #[inline]
    pub fn pass_hash(self,key:u64)->u64 {key^SIDE_KEY}
    #[inline]
    pub fn child_hash(self,key:u64,m:Move)->u64 {
        let placed=(1u64<<m.a)|(1u64<<m.b);
        // Captures include partners that already have the mover's colour.
        let bd=if self.side==1 {placed|(m.flips&self.white)} else {m.flips&self.black};
        let wd=if self.side == -1 {placed|(m.flips&self.black)} else {m.flips&self.white};
        key ^ fingerprint(bd,0)^fingerprint(wd,1)^fingerprint(1u64<<m.a,if m.b-m.a==1 {2} else {3})^SIDE_KEY
    }
    pub fn sequence(text: &str) -> Result<Self, String> {
        let mut p = Self::initial();
        for token in text
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|x| !x.is_empty())
        {
            let mut ms = p.moves();
            if token == "pass" {
                if !ms.is_empty() || p.pass().moves().is_empty() {
                    return Err("Illegal pass".into());
                }
                p = p.pass();
                continue;
            }
            if ms.is_empty() {
                p = p.pass();
                ms = p.moves();
            }
            let t = token.to_ascii_lowercase();
            let m = ms
                .into_iter()
                .find(|m| m.to_string() == t)
                .ok_or_else(|| format!("Illegal move: {token}"))?;
            p = p.play(m);
        }
        Ok(p)
    }
    pub fn validate(self) -> Result<Self, String> {
        if self.side != 1 && self.side != -1 {
            return Err("turn must be 1 or -1".into());
        }
        if self.black & self.white != 0
            || self.hleft & !NOT_H != 0
            || self.vtop & 0xff00000000000000 != 0
        {
            return Err("Invalid bitboards".into());
        }
        let a = self.hleft;
        let b = a << 1;
        let c = self.vtop;
        let d = c << 8;
        if (a | b | c | d) != (self.black | self.white)
            || a & b != 0
            || a & c != 0
            || a & d != 0
            || b & c != 0
            || b & d != 0
            || c & d != 0
        {
            return Err("Invalid pairing".into());
        }
        Ok(self)
    }
}
pub fn perft(p: Position, d: u32) -> u64 {
    if d == 0 {
        return 1;
    }
    let ms = p.move_list();
    if ms.is_empty() {
        return if p.pass().mobility()==0 {
            1
        } else {
            perft(p.pass(), d - 1)
        };
    }
    ms.iter().map(|m| perft(p.play(*m), d - 1)).sum()
}
pub fn random_position(seed: &mut u64, empties: u32) -> Position {
    let mut p = Position::initial();
    while p.empty().count_ones() > empties {
        let ms = p.moves();
        if ms.is_empty() {
            p = p.pass();
            if p.moves().is_empty() {
                break;
            }
            continue;
        }
        *seed ^= *seed << 13;
        *seed ^= *seed >> 7;
        *seed ^= *seed << 17;
        p = p.play(ms[*seed as usize % ms.len()]);
    }
    p
}

const SIDE_KEY:u64=0x9e3779b97f4a7c15;
#[inline(always)]
fn fingerprint(x:u64,kind:usize)->u64 {
    match kind {
        0=>x^x.rotate_left(7)^x.rotate_left(19)^x.rotate_left(43)^x.rotate_left(53),
        1=>x.rotate_left(3)^x.rotate_left(13)^x.rotate_left(29)^x.rotate_left(37)^x.rotate_left(59),
        2=>x.rotate_left(5)^x.rotate_left(17)^x.rotate_left(31),
        _=>x.rotate_left(11)^x.rotate_left(23)^x.rotate_left(47),
    }
}

impl Position {
    /// Conservative immutable edge set, closed under domino partners.
    /// Recompute after partner pruning: an unsupported corner must not anchor
    /// the rest of its edge. This also handles mixed-colour imported pairs.
    pub fn stable_edges(self,colour:u64)->u64 {
        let mut allowed=colour;
        loop {
            let mut protected=0;
            for (corner,steps) in [(0,[1,8]),(7,[-1,8]),(56,[1,-8]),(63,[-1,-8])] {
                if allowed & (1u64<<corner)==0 {continue}
                protected|=1u64<<corner;
                for step in steps {for k in 1..8 {
                    let bit=1u64<<((corner as i32+step*k) as u32);
                    if allowed&bit==0 {break} protected|=bit;
                }}
            }
            let next=protected & self.partners(protected);
            if next==allowed {return next}
            allowed=next;
        }
    }
    /// Every domino consumes one square of each checkerboard colour within
    /// one empty component. This is an upper bound (not a matching algorithm).
    pub fn occupancy_bound(self)->i32 {
        let mut remaining=self.empty();let mut max=(!remaining).count_ones();
        while remaining!=0 {
            let mut region=remaining & remaining.wrapping_neg();
            loop {let next=region | ((shift(region,1)|shift(region,3)|shift(region,4)|shift(region,6))&remaining);
                if next==region {break} region=next;
            }
            let a=(region&0xaa55aa55aa55aa55).count_ones();
            max+=2*a.min(region.count_ones()-a);remaining &= !region;
        }
        max as i32
    }
}
