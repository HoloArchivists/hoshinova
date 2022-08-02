import { useQuery } from '@tanstack/react-query';

export const useQueryConfig = () =>
  useQuery(['config'], () => fetch('/api/config').then((res) => res.json()));
