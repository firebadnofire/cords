export type ConnectionState = 'idle' | 'connecting' | 'connected' | 'failed';

export function connectionLabel(state: ConnectionState): string {
  switch (state) {
    case 'idle':
      return 'Not connected';
    case 'connecting':
      return 'Inspecting server';
    case 'connected':
      return 'Metadata verified';
    case 'failed':
      return 'Connection failed';
  }
}
