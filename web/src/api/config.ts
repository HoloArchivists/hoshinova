import { useMutation, useQuery } from '@tanstack/react-query';

export const useQueryConfig = () =>
  useQuery(['config'], () => fetch('/api/config').then((res) => res.json()));

export const useMutateReloadConfig = () =>
  useMutation(() =>
    fetch('/api/config/reload', { method: 'POST' }).then((res) => res.json())
  );
