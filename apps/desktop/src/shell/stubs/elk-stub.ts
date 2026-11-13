// Pengganti `elkjs` (EPL-2.0/GPL, bukan lisensi permisif; ADR-035). Mermaid hanya memakainya untuk tata letak "elk"
// yang opsional; diagram default (dagre) tidak terpengaruh. Memakai tata letak elk menghasilkan galat yang jelas.
export default class ELKUnavailable {
  layout(): Promise<never> {
    return Promise.reject(new Error("tata letak ELK tidak tersedia di Zandor (lisensi); gunakan tata letak bawaan"));
  }
}
