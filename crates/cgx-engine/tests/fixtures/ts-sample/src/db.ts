export async function query(sql: string, params: any[] = []): Promise<any[]> {
  return [];
}

export function getConnection() {
  return { connected: true };
}
