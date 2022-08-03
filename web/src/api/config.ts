import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

const rejectError = (res: Response) =>
  res.status >= 200 && res.status < 300
    ? Promise.resolve(res)
    : Promise.reject(res);

export const useQueryConfig = () =>
  useQuery(['config'], () =>
    fetch('/api/config')
      .then(rejectError)
      .then((res) => res.json())
  );

export const useMutateReloadConfig = () => {
  const queryClient = useQueryClient();
  return useMutation(
    () =>
      fetch('/api/config/reload', { method: 'POST' })
        .then(rejectError)
        .then((res) => res.json()),
    {
      onSuccess: () => {
        queryClient.invalidateQueries(['config']);
        queryClient.invalidateQueries(['config', 'toml']);
      },
    }
  );
};

export const useQueryConfigTOML = () =>
  useQuery(['config', 'toml'], () =>
    fetch('/api/config/toml')
      .then(rejectError)
      .then((res) => res.text())
  );

export const useMutateConfigTOML = () => {
  const queryClient = useQueryClient();
  return useMutation(
    (toml: string) =>
      fetch('/api/config/toml', {
        method: 'PUT',
        body: toml,
        headers: { 'Content-Type': 'text/toml' },
      })
        .then(rejectError)
        .then((res) => res.text()),
    {
      onSuccess: () => {
        queryClient.invalidateQueries(['config']);
        queryClient.invalidateQueries(['config', 'toml']);
      },
    }
  );
};
