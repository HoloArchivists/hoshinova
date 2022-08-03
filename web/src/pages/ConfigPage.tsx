import { Button, Code, Container, Group, Text } from '@mantine/core';
import { useMutateReloadConfig, useQueryConfig } from '../api/config';

const ConfigPage = () => {
  const qConfig = useQueryConfig();
  const mReload = useMutateReloadConfig();

  return (
    <Container fluid py="md">
      <Code block>{JSON.stringify(qConfig.data, null, 2)}</Code>
      <Group pt="md">
        <Button onClick={() => mReload.mutate()} disabled={mReload.isLoading}>
          Reload configuration
        </Button>
        <Text>Configuration editing coming soon(tm)</Text>
      </Group>
    </Container>
  );
};

export default ConfigPage;
