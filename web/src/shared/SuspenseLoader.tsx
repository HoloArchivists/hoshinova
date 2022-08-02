import { Container, AspectRatio, LoadingOverlay } from '@mantine/core';

export const SuspenseLoader = () => (
  <Container size="xs">
    <AspectRatio ratio={1}>
      <LoadingOverlay visible overlayOpacity={0} />
    </AspectRatio>
  </Container>
);
