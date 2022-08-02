import { Code, Container, Text } from '@mantine/core';
import { useQueryConfig } from '../api/config';

const ConfigPage = () => {
  const qConfig = useQueryConfig();

  return (
    <Container fluid py="md">
      <Code block>{JSON.stringify(qConfig.data, null, 2)}</Code>
      <Text pt="md">Configuration editing coming soon(tm)</Text>
    </Container>
  );
};

export default ConfigPage;
