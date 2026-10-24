import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from 'react';
import { ApiError, onUnauthenticated } from './api/client';
import * as accounts from './api/accounts';
import type { Account } from './api/types';

type AuthState =
  | { status: 'loading' }
  | { status: 'anonymous' }
  | { status: 'authenticated'; account: Account }
  | { status: 'error'; error: unknown };

interface AuthApi {
  state: AuthState;
  /** Store the account returned by sign-in, sign-up or verification. */
  setAccount: (account: Account) => void;
  /** Re-read who is signed in. */
  refresh: () => Promise<void>;
  signOut: () => Promise<void>;
  signOutEverywhere: () => Promise<void>;
}

const AuthContext = createContext<AuthApi | null>(null);

/** Asks the server who is signed in. 401 means nobody; anything else is a connection problem. */
async function readState(): Promise<AuthState> {
  try {
    return { status: 'authenticated', account: await accounts.getMe() };
  } catch (error) {
    if (error instanceof ApiError && error.status === 401) return { status: 'anonymous' };
    return { status: 'error', error };
  }
}

export function AuthProvider({ children }: { children: ReactNode }) {
  const [state, setState] = useState<AuthState>({ status: 'loading' });

  const refresh = useCallback(async () => {
    setState(await readState());
  }, []);

  // The first answer arrives asynchronously, so state is set in the callback, not the effect.
  useEffect(() => {
    let active = true;
    void readState().then((next) => {
      if (active) setState(next);
    });
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    onUnauthenticated(() => setState({ status: 'anonymous' }));
    return () => onUnauthenticated(null);
  }, []);

  const api = useMemo<AuthApi>(
    () => ({
      state,
      setAccount: (account) => setState({ status: 'authenticated', account }),
      refresh,
      signOut: async () => {
        await accounts.signOut();
        setState({ status: 'anonymous' });
      },
      signOutEverywhere: async () => {
        await accounts.signOutEverywhere();
        setState({ status: 'anonymous' });
      },
    }),
    [state, refresh],
  );

  return <AuthContext.Provider value={api}>{children}</AuthContext.Provider>;
}

export function useAuth(): AuthApi {
  const value = useContext(AuthContext);
  if (!value) throw new Error('useAuth must be used inside AuthProvider');
  return value;
}
