import { Outlet } from "react-router-dom";
import { Toaster } from "react-hot-toast";
import { WalletKitProvider } from "@mysten/wallet-kit";
import { ReactQueryDevtools } from '@tanstack/react-query-devtools';

export function Root() {
  return (
    <WalletKitProvider enableUnsafeBurner={import.meta.env.DEV}>
      <Outlet />
      <Toaster />
      {import.meta.env.DEV && <ReactQueryDevtools />}
    </WalletKitProvider>
  );
}
