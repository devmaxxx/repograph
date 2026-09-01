/** Money in grosze; see FR-PAY-03 and INV-11. */
export function asGrosze(v: number): number {
  return Math.round(v * 100);
}
export const zero = asGrosze(0);
function internal() {}
