// URL overrides the saved preference; evaluations are hidden by default.
export function resolveAnalysis(search,stored){const value=new URLSearchParams(search).get('analysis');return (value==='1'||value==='0'?value:stored)==='1';}
